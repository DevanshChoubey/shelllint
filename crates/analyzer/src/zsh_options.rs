// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint extension, no ShellCheck counterpart. A separate
// analysis pass, run only for zsh scripts: it walks the tree once, in source
// order, and records for every node the zsh option state in effect there.
// `Params::zsh_option(node, opt)` reads the result.
//
// What it models (design spec section 3, research Q4; every rule below was
// checked against zsh 5.9 with `zsh -fc`, the evidence sits next to the tests
// in `crates/analyzer/tests/zsh_options.rs`):
//
// * `setopt`, `unsetopt`, `set -o` / `set +o` (also inside a flag cluster such
//   as `-eo`), `set -y` / `set -3`, `emulate [-L] [-R] zsh|sh|ksh|bash
//   [-o opt]...` and `options[opt]=on|off`.
// * Spelling: case is ignored, underscores are ignored, a `no` prefix negates
//   (`unsetopt noX` sets X, `options[noX]=on` turns X off), unknown options
//   are ignored.
// * Scope: a function restores the options on return only if `localoptions`
//   is on when its body ends (`setopt localoptions` anywhere in the body, or
//   `emulate -L`; a later `nolocaloptions` cancels it). `if`, `while`, `for`
//   and `{ }` do not scope. Subshells, command substitutions, process
//   substitutions, background jobs and every pipeline element but the last
//   do, because they run in a forked shell.
// * Static, along the code: a function body starts with the state at its
//   definition. A call of a function defined earlier in the file applies the
//   options that function leaves behind (nothing if it restores them), so a
//   leak reaches the code after the call; a helper defined further down the
//   file than a call inside another function body is found by a second pass.
//   Dynamic, caller-dependent state is not modelled: a function does not see
//   options its caller changes after the definition. `a && f` and `if` are
//   walked straight through, as if every branch ran.
// * Every file starts with the zsh defaults.
//
// For non-zsh shells this pass is never run.
use crate::analyzerlib::ZshOpt;
use shelllint_core::ast::{AssignmentMode, Ast, Id, Token};
use shelllint_core::astlib::get_literal_string;
use std::collections::HashMap;

/// Every option the tracker follows, in bit order.
const ALL: [ZshOpt; 14] = [
    ZshOpt::ShWordSplit,
    ZshOpt::KshArrays,
    ZshOpt::GlobSubst,
    ZshOpt::RcExpandParam,
    ZshOpt::BsdEcho,
    ZshOpt::PromptSubst,
    ZshOpt::ExtendedGlob,
    ZshOpt::KshGlob,
    ZshOpt::ShGlob,
    ZshOpt::NullGlob,
    ZshOpt::NoMatch,
    ZshOpt::LocalOptions,
    ZshOpt::ErrReturn,
    ZshOpt::PipeFail,
];

fn bit(opt: ZshOpt) -> u16 {
    let index = ALL.iter().position(|&o| o == opt).expect("every ZshOpt is in ALL");
    1 << index
}

/// zsh 5.9 defaults (`zsh -fc`): only `nomatch` is on.
const DEFAULT_STATE: u16 = 1 << 10;

const LOCAL: u16 = 1 << 11;
const PROMPT_SUBST: u16 = 1 << 5;

/// Names (case and underscores ignored) to options. `promptvars` is zsh's
/// alias of `promptsubst`.
fn lookup_exact(name: &str) -> Option<ZshOpt> {
    Some(match name {
        "shwordsplit" => ZshOpt::ShWordSplit,
        "ksharrays" => ZshOpt::KshArrays,
        "globsubst" => ZshOpt::GlobSubst,
        "rcexpandparam" => ZshOpt::RcExpandParam,
        "bsdecho" => ZshOpt::BsdEcho,
        "promptsubst" | "promptvars" => ZshOpt::PromptSubst,
        "extendedglob" => ZshOpt::ExtendedGlob,
        "kshglob" => ZshOpt::KshGlob,
        "shglob" => ZshOpt::ShGlob,
        "nullglob" => ZshOpt::NullGlob,
        "nomatch" => ZshOpt::NoMatch,
        "localoptions" => ZshOpt::LocalOptions,
        "errreturn" => ZshOpt::ErrReturn,
        "pipefail" => ZshOpt::PipeFail,
        _ => return None,
    })
}

fn normalize(name: &str) -> String {
    name.chars().filter(|&c| c != '_').map(|c| c.to_ascii_lowercase()).collect()
}

/// An option name as written after `setopt`: the option and whether the `no`
/// prefix negated it. The exact name wins (`nomatch` is the option itself,
/// `nonomatch` its negation).
fn parse_option(name: &str) -> Option<(ZshOpt, bool)> {
    let n = normalize(name);
    if let Some(o) = lookup_exact(&n) {
        return Some((o, false));
    }
    n.strip_prefix("no").and_then(lookup_exact).map(|o| (o, true))
}

/// The net effect of a stretch of code on the options: for each bit in `mask`,
/// the value it ends up with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Effect {
    mask: u16,
    vals: u16,
}

impl Effect {
    fn set(&mut self, opt: ZshOpt, on: bool) {
        let b = bit(opt);
        self.mask |= b;
        if on {
            self.vals |= b;
        } else {
            self.vals &= !b;
        }
    }

    fn apply(self, state: u16) -> u16 {
        (state & !self.mask) | (self.vals & self.mask)
    }

    /// `self` followed by `later`.
    fn then(self, later: Effect) -> Effect {
        Effect { mask: self.mask | later.mask, vals: (self.vals & !later.mask) | (later.vals & later.mask) }
    }
}

/// The option state recorded for every node of one tree.
#[derive(Clone, Debug)]
pub struct ZshOptionMap {
    states: Vec<u16>,
}

impl ZshOptionMap {
    /// Whether `opt` is on at `node`. A node the pass did not see (an id from
    /// another tree) reads as the zsh default.
    pub fn get(&self, node: Id, opt: ZshOpt) -> bool {
        let state = self.states.get(node.0 as usize).copied().unwrap_or(DEFAULT_STATE);
        state & bit(opt) != 0
    }
}

/// Build the option-state map of the tree under `root`.
pub fn build(ast: &Ast, root: Id) -> ZshOptionMap {
    let run = |later: HashMap<String, Effect>| {
        let mut t = Tracker {
            ast,
            state: DEFAULT_STATE,
            frame: Effect::default(),
            funcs: HashMap::new(),
            later,
            states: vec![DEFAULT_STATE; ast.len() as usize],
        };
        t.walk(root, 0);
        t
    };
    let first = run(HashMap::new());
    // A function body can call a helper defined further down. If any function
    // leaves options behind, walk once more with those effects known up front
    // (used only for names not yet defined where the call is). Scripts where no
    // function changes options, nearly all of them, pay for one pass only.
    let leaking: HashMap<String, Effect> = first.funcs.into_iter().filter(|(_, e)| e.mask != 0).collect();
    if leaking.is_empty() {
        return ZshOptionMap { states: first.states };
    }
    ZshOptionMap { states: run(leaking).states }
}

/// Deeper than this the walk stops following scope rules and just labels the
/// rest of the subtree with the state it reached, so a pathological nesting
/// cannot overflow the stack.
const MAX_DEPTH: usize = 200;

struct Tracker<'a> {
    ast: &'a Ast,
    state: u16,
    /// What the function body being walked has done to the options so far.
    frame: Effect,
    /// Net effect of each function defined so far, by name.
    funcs: HashMap<String, Effect>,
    /// Effects of functions defined further down the file, known from an
    /// earlier pass. Consulted only for a name with no definition yet.
    later: HashMap<String, Effect>,
    states: Vec<u16>,
}

impl Tracker<'_> {
    fn change(&mut self, e: Effect) {
        self.state = e.apply(self.state);
        self.frame = self.frame.then(e);
    }

    fn set_opt(&mut self, opt: ZshOpt, on: bool) {
        let mut e = Effect::default();
        e.set(opt, on);
        self.change(e);
    }

    /// Run `f`, then forget whatever it did to the options (a forked shell).
    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        let saved = (self.state, self.frame);
        f(self);
        (self.state, self.frame) = saved;
    }

    fn label_subtree(&mut self, id: Id) {
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            self.states[n.0 as usize] = self.state;
            stack.extend(self.ast.children(n));
        }
    }

    fn walk(&mut self, id: Id, depth: usize) {
        if depth > MAX_DEPTH {
            self.label_subtree(id);
            return;
        }
        self.states[id.0 as usize] = self.state;
        match self.ast.get(id) {
            Token::T_Function(_, _, name, body) => self.function(name, *body, depth),
            Token::T_Subshell(_)
            | Token::T_Backgrounded(_)
            | Token::T_DollarExpansion(_)
            | Token::T_Backticked(_)
            | Token::T_ProcSub(..) => self.scoped(|t| t.children(id, depth)),
            Token::T_Pipeline(_, cmds) => {
                let last = cmds.len().saturating_sub(1);
                for (i, &c) in cmds.clone().iter().enumerate() {
                    if i == last {
                        self.walk(c, depth + 1);
                    } else {
                        self.scoped(|t| t.walk(c, depth + 1));
                    }
                }
            }
            Token::T_SimpleCommand(assigns, words) => {
                self.children(id, depth);
                self.simple_command(assigns, words);
            }
            _ => self.children(id, depth),
        }
    }

    fn children(&mut self, id: Id, depth: usize) {
        for c in self.ast.children(id) {
            self.walk(c, depth + 1);
        }
    }

    /// A function definition: the body is walked from the state at the
    /// definition, and what it leaves behind is remembered for its calls. An
    /// anonymous function needs no case of its own: the parser (lane P-A)
    /// builds `() { body } args` as a definition named
    /// `zsh-anonymous-function` followed by an immediate call of that name.
    fn function(&mut self, name: &str, body: Id, depth: usize) {
        let saved = (self.state, self.frame);
        self.frame = Effect::default();
        self.walk(body, depth + 1);
        let mut leaves = if self.state & LOCAL != 0 { Effect::default() } else { self.frame };
        // `localoptions` itself is always back to its value at the call
        // (zsh -fc 'emulate -L zsh; f(){ unsetopt localoptions; setopt nullglob }; f'
        // leaves nullglob on and localoptions on).
        leaves.mask &= !LOCAL;
        leaves.vals &= !LOCAL;
        (self.state, self.frame) = saved;
        for n in name.split_whitespace() {
            self.funcs.insert(n.to_string(), leaves);
        }
    }

    fn literal(&self, id: Id) -> Option<String> {
        get_literal_string(self.ast, id)
    }

    fn simple_command(&mut self, assigns: &[Id], words: &[Id]) {
        if words.is_empty() {
            for &a in assigns {
                self.options_assignment(a);
            }
            return;
        }
        let mut i = 0;
        let mut builtin_only = false;
        while let Some(w) = words.get(i).and_then(|&w| self.literal(w)) {
            match w.as_str() {
                "builtin" => builtin_only = true,
                "noglob" | "nocorrect" => {}
                _ => break,
            }
            i += 1;
        }
        let Some(name) = words.get(i).and_then(|&w| self.literal(w)) else { return };
        let args: Vec<Option<String>> = words[i + 1..].iter().map(|&w| self.literal(w)).collect();
        if !builtin_only && let Some(&e) = self.funcs.get(&name).or_else(|| self.later.get(&name)) {
            self.change(e);
            return;
        }
        match name.as_str() {
            "setopt" => self.setopt(&args, true),
            "unsetopt" => self.setopt(&args, false),
            "set" => self.set(&args),
            "emulate" => self.emulate(&args),
            _ => {}
        }
    }

    fn apply_named(&mut self, name: &str, on: bool) {
        if let Some((opt, negated)) = parse_option(name) {
            self.set_opt(opt, on != negated);
        }
    }

    /// `setopt a b`, `setopt -o a`, `setopt +o a` (`+o` inverts the next
    /// name; `-o`/`+o` take one name each, plain names around them keep the
    /// command's own sense). `base` is true for `setopt`, false for `unsetopt`.
    fn setopt(&mut self, args: &[Option<String>], base: bool) {
        let mut pending: Option<bool> = None;
        for arg in args {
            let Some(arg) = arg else {
                pending = None;
                continue;
            };
            if let Some(invert) = pending.take() {
                self.apply_named(arg, base != invert);
                continue;
            }
            match arg.as_str() {
                "-o" => pending = Some(false),
                "+o" => pending = Some(true),
                // `-m` takes patterns, which are not followed.
                "-m" => break,
                a if a.len() > 1 && (a.starts_with('-') || a.starts_with('+')) => {}
                _ => self.apply_named(arg, base),
            }
        }
    }

    /// `set -o a`, `set +o a`, flag clusters (`-eo a`), `-y`/`+y`
    /// (`shwordsplit`) and `-3`/`+3` (`nomatch`, inverted). The flags end at
    /// the first argument that is not one, so `set -o a b` leaves `b` a
    /// positional parameter.
    fn set(&mut self, args: &[Option<String>]) {
        let mut i = 0;
        while let Some(Some(arg)) = args.get(i) {
            i += 1;
            if arg == "--" {
                break;
            }
            let mut chars = arg.chars();
            let on = match chars.next() {
                Some('-') => true,
                Some('+') => false,
                _ => break,
            };
            for c in chars {
                match c {
                    'o' => {
                        if let Some(Some(name)) = args.get(i) {
                            self.apply_named(name, on);
                        }
                        i += 1;
                    }
                    'y' => self.set_opt(ZshOpt::ShWordSplit, on),
                    '3' => self.set_opt(ZshOpt::NoMatch, !on),
                    // `-A name value...` and `-s` take the rest as data.
                    'A' | 's' => return,
                    _ => {}
                }
            }
        }
    }

    /// `emulate [-L] [-R] shell [-o opt]...`. The bundle first, then the
    /// `-o` options, then `-L`'s `localoptions` (zsh applies them in that
    /// order: `emulate -L zsh -o nolocaloptions` leaves it on). Without `-R`
    /// `promptsubst` is left alone (an interactive-environment option).
    /// `-c cmd` runs the command under the emulation and restores it, so it
    /// changes nothing here.
    fn emulate(&mut self, args: &[Option<String>]) {
        let (mut local, mut reset_all) = (false, false);
        let mut shell: Option<String> = None;
        let mut extra: Vec<(String, bool)> = Vec::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            // zsh rejects the whole command on an argument it does not know, so
            // nothing changes; an expansion is just as unknown to a linter.
            let Some(arg) = arg else { return };
            match arg.as_str() {
                "-o" | "+o" => {
                    // `emulate -o x zsh` is an error: the shell comes first.
                    if shell.is_none() {
                        return;
                    }
                    if let Some(Some(name)) = it.next() {
                        extra.push((name.clone(), arg == "-o"));
                    }
                }
                flags if flags.len() > 1 && flags.starts_with('-') => {
                    // `-c cmd` runs cmd under the emulation and restores it.
                    if flags.contains('c') {
                        return;
                    }
                    local |= flags.contains('L');
                    reset_all |= flags.contains('R');
                }
                _ if shell.is_none() => shell = Some(arg.clone()),
                _ => return,
            }
        }
        let Some(shell) = shell else { return };
        let mut e = Effect::default();
        match shell.as_str() {
            "zsh" => {
                for o in ALL {
                    e.set(o, o == ZshOpt::NoMatch);
                }
            }
            // `emulate bash` behaves as `emulate sh` in zsh 5.9.
            "sh" | "bash" | "ksh" => {
                let ksh = shell == "ksh";
                for o in ALL {
                    let on = match o {
                        ZshOpt::ShWordSplit | ZshOpt::KshArrays | ZshOpt::GlobSubst | ZshOpt::ShGlob => true,
                        ZshOpt::BsdEcho => !ksh,
                        ZshOpt::KshGlob | ZshOpt::LocalOptions => ksh,
                        _ => false,
                    };
                    e.set(o, on);
                }
            }
            _ => return,
        }
        if shell != "zsh" && reset_all {
            e.set(ZshOpt::PromptSubst, true);
        }
        if shell == "zsh" && reset_all {
            e.set(ZshOpt::PromptSubst, false);
        }
        if !reset_all {
            e.mask &= !PROMPT_SUBST;
            e.vals &= !PROMPT_SUBST;
        }
        self.change(e);
        for (name, on) in extra {
            self.apply_named(&name, on);
        }
        if local {
            self.set_opt(ZshOpt::LocalOptions, true);
        }
    }

    /// `options[name]=on|off`: the value is exactly `on` or `off`, and the
    /// name is spelled as for `setopt`, so `options[noX]=on` turns X off.
    fn options_assignment(&mut self, assignment: Id) {
        let Token::T_Assignment(AssignmentMode::Assign, var, indices, value) = self.ast.get(assignment) else {
            return;
        };
        if var != "options" || indices.len() != 1 {
            return;
        }
        let (Some(index), Some(value)) = (self.index_text(indices[0]), self.literal(*value)) else { return };
        let on = match value.as_str() {
            "on" => true,
            "off" => false,
            _ => return,
        };
        self.apply_named(&index, on);
    }

    /// The text of an array index that is a plain word.
    fn index_text(&self, id: Id) -> Option<String> {
        match self.ast.get(id) {
            Token::T_UnparsedIndex(_, s) => Some(s.clone()),
            Token::T_Literal(s) => Some(s.clone()),
            Token::TA_Variable(name, idx) if idx.is_empty() => Some(name.clone()),
            Token::TA_Sequence(v) | Token::TA_Expansion(v) => {
                let mut out = String::new();
                for &p in v {
                    out.push_str(&self.index_text(p)?);
                }
                Some(out)
            }
            Token::T_NormalWord(_) => self.literal(id),
            _ => None,
        }
    }
}
