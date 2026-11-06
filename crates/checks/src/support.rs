// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the shared check helpers of ShellCheck (e11f964):
// src/ShellCheck/Analytics.hs (`runNodeAnalysis`,
// `nodeChecksToTreeCheck`, `checkCommand`, `hasFloatingPoint`, `isCondition`,
// `replaceStart`, `replaceEnd`, `replaceToken`, `surroundWith`, `fixWith`,
// `analyse`, `functions`, `doVariableFlowAnalysis`,
// `quotesMayConflictWithSC2281`, `addDoubleQuotesAround`,
// `checkSpacefulnessCfg'`, `isSourced`) and
// src/ShellCheck/Checks/Commands.hs (`checkGrepSendsPipefailImpl`,
// `returnOrExit`, `getSingleUnmodifiedBracedString`, `missingDestination`).
//
// These are the `where`-less top-level helpers used by two or more checks
// (corpus/checks.json `helpers`). A helper used by one check is ported in
// that check's file instead. Definitions follow the Haskell order, Analytics
// first, then Commands.
//
// Plus `Scratch`, the shared way to build synthetic tokens (see its doc and
// docs/porting-a-check.md).

use std::collections::{BTreeMap, HashMap};

use shellcheck_analyzer::analyzerlib::{
    Comments, DataType, Params, StackData, is_command, is_counting_reference, is_param_to, is_quote_free,
    is_quoted_alternative_reference, used_as_command_name,
};
use shellcheck_analyzer::cfg_analysis::{CFVariableProp, SpaceStatus, VariableState, get_incoming_state};
use shellcheck_core::ast::{Ast, Id, Pos, Token};
use shellcheck_core::astlib::{
    arguments, brace_expand, get_all_flags, get_braced_modifier, get_braced_reference, get_literal_string_ext,
    get_opts, get_path, get_word_parts, is_array_expansion, may_become_multiple_args, oversimplify,
};
use shellcheck_core::data::SPECIAL_VARIABLES_WITHOUT_SPACES;
use shellcheck_core::interface::{Fix, InsertionPoint, Position, Replacement, Shell, TokenComment};

// ---- Analytics.hs ----

/// `runNodeAnalysis f p t`: run a node check at every node of `t`, in
/// pre-order, and collect what it writes.
pub fn run_node_analysis(f: &dyn Fn(&Params<'_>, Id, &mut Comments), p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    let mut c = Comments::new();
    p.ast.analyze(t, &mut |x| f(p, x, &mut c), &mut |_| {});
    c.into_vec()
}

/// `nodeChecksToTreeCheck checkList`: one walk running every node check at
/// each node.
pub use shellcheck_analyzer::api::node_checks_to_tree_check;

/// `checkCommand str f t`: if `t` is the command `str` (or `.../str`), run
/// `f cmd rest` on its name word and arguments.
pub fn check_command(ast: &Ast, str: &str, f: impl FnOnce(Id, &[Id]), t: Id) {
    if let Token::T_SimpleCommand(_, words) = ast.get(t)
        && let Some((&cmd, rest)) = words.split_first()
        && is_command(ast, t, str)
    {
        f(cmd, rest);
    }
}

/// `hasFloatingPoint params = shellType params == Ksh`
pub fn has_floating_point(params: &Params<'_>) -> bool {
    params.shell_type == Shell::Ksh
}

/// Checks whether the current parent path is part of a condition.
/// `path` is a `getPath` result: the token first, then its parents.
pub fn is_condition(ast: &Ast, path: &[Id]) -> bool {
    // foldr go (const False) xs x: walk (child, parent) pairs upwards.
    path.windows(2).any(|w| {
        let (child, parent) = (w[0], w[1]);
        // count anything in a @test as conditional
        matches!(ast.get(child), Token::T_BatsTest(..)) || get_condition_children(ast, parent).contains(&child)
    })
}

fn get_condition_children(ast: &Ast, t: Id) -> Vec<Id> {
    match ast.get(t) {
        Token::T_AndIf(left, _right) => vec![*left],
        Token::T_OrIf(left, _right) => vec![*left],
        Token::T_IfExpression(conditions, _elses) => conditions.iter().filter_map(|(c, _)| c.last().copied()).collect(),
        Token::T_WhileExpression(c, _l) => c.last().copied().into_iter().collect(),
        Token::T_UntilExpression(c, _l) => c.last().copied().into_iter().collect(),
        _ => vec![],
    }
}

// DIVERGENCE: the arena stores `Pos` without a file name, so replacement
// positions have an empty `file`, where Haskell's carry the token's source
// file. Whoever turns fixes into positioned output must fill it in.
fn position(p: Pos) -> Position {
    Position { file: String::new(), line: i64::from(p.line), column: i64::from(p.column) }
}

/// `length $ getPath (parentMap params) (T_EOF id)`: the token's depth.
fn depth(params: &Params<'_>, id: Id) -> i64 {
    get_path(params.ast, id).len() as i64
}

// helpers to build replacements

/// Replace the first `n` columns of token `id` with `r` (`n = 0` inserts
/// before it).
pub fn replace_start(id: Id, params: &Params<'_>, n: i64, r: &str) -> Replacement {
    let (start, _) = params.ast.span(id);
    let start = position(start);
    let new_end = Position { column: start.column + n, ..start.clone() };
    Replacement {
        start_pos: start,
        end_pos: new_end,
        string: r.to_string(),
        precedence: depth(params, id),
        insertion_point: InsertionPoint::InsertAfter,
    }
}

/// Replace the last `n` columns of token `id` with `r` (`n = 0` appends).
pub fn replace_end(id: Id, params: &Params<'_>, n: i64, r: &str) -> Replacement {
    let (_, end) = params.ast.span(id);
    let end = position(end);
    let new_start = Position { column: end.column - n, ..end.clone() };
    let new_end = Position { column: end.column, ..end };
    Replacement {
        start_pos: new_start,
        end_pos: new_end,
        string: r.to_string(),
        precedence: depth(params, id),
        insertion_point: InsertionPoint::InsertBefore,
    }
}

/// Replace the whole token `id` with `r`.
pub fn replace_token(id: Id, params: &Params<'_>, r: &str) -> Replacement {
    let (start, end) = params.ast.span(id);
    Replacement {
        start_pos: position(start),
        end_pos: position(end),
        string: r.to_string(),
        precedence: depth(params, id),
        insertion_point: InsertionPoint::InsertBefore,
    }
}

/// Put `s` before and after token `id`.
pub fn surround_with(id: Id, params: &Params<'_>, s: &str) -> Fix {
    fix_with(vec![replace_start(id, params, 0, s), replace_end(id, params, 0, s)])
}

/// `fixWith fixes = newFix { fixReplacements = fixes }`
pub fn fix_with(fixes: Vec<Replacement>) -> Fix {
    Fix { replacements: fixes }
}

/// `analyse f t = execState (doAnalysis f t) []`: run `f` at every node in
/// pre-order, threading a list state that `f` prepends to.
// DIVERGENCE: `State [a]` becomes a `Vec` that `f` pushes to, so the result
// is in visiting order; Haskell's (built with `modify (x:)`) is reversed.
pub fn analyse<A>(ast: &Ast, f: &mut dyn FnMut(Id, &mut Vec<A>), t: Id) -> Vec<A> {
    let mut state = Vec::new();
    ast.analyze(t, &mut |x| f(x, &mut state), &mut |_| {});
    state
}

/// Make a map from functions to definition IDs.
// `Map.fromList` over the reversed (prepended) list keeps the first
// definition in pre-order for each name.
pub fn functions(ast: &Ast, t: Id) -> BTreeMap<String, Id> {
    let mut map = BTreeMap::new();
    for (name, id) in analyse(ast, &mut |x, st| find_functions(ast, x, st), t) {
        map.entry(name).or_insert(id);
    }
    map
}

fn find_functions(ast: &Ast, t: Id, st: &mut Vec<(String, Id)>) {
    if let Token::T_Function(_, _, name, _) = ast.get(t) {
        st.push((name.clone(), t));
    }
}

/// For each variable read/write, run a stateful function that emits
/// comments. The comments are collected and returned.
///
/// `read_func(state, base, token, name)` and
/// `write_func(state, base, token, name, values)`; `empty` is the initial
/// state.
// `foldM (\list x -> do { l <- doFlow x; return $ l ++ list }) []`: each
// step's output goes in front, so later events come first.
pub fn do_variable_flow_analysis<S, V>(
    mut read_func: impl FnMut(&mut S, Id, Id, &str) -> Vec<V>,
    mut write_func: impl FnMut(&mut S, Id, Id, &str, &DataType) -> Vec<V>,
    empty: S,
    flow: &[StackData],
) -> Vec<V> {
    let mut state = empty;
    let mut chunks: Vec<Vec<V>> = Vec::new();
    for x in flow {
        let l = match x {
            StackData::Reference(base, token, name) => read_func(&mut state, *base, *token, name),
            StackData::Assignment(base, token, name, values) => write_func(&mut state, *base, *token, name, values),
            _ => vec![],
        };
        chunks.push(l);
    }
    chunks.into_iter().rev().flatten().collect()
}

// Don't suggest quotes if this will instead be autocorrected
// from $foo=bar to foo=bar. This is not pretty but ok.
pub fn quotes_may_conflict_with_sc2281(params: &Params<'_>, t: Id) -> bool {
    let path = get_path(params.ast, t);
    let [_, w, sc, ..] = &path[..] else { return false };
    let Token::T_NormalWord(parts) = params.ast.get(*w) else { return false };
    let [me, lit, ..] = &parts[..] else { return false };
    let Token::T_Literal(s) = params.ast.get(*lit) else { return false };
    if !s.starts_with('=') {
        return false;
    }
    let Token::T_SimpleCommand(_, words) = params.ast.get(*sc) else { return false };
    let Some(cmd) = words.first() else { return false };
    t == *me && *w == *cmd
}

pub fn add_double_quotes_around(params: &Params<'_>, token: Id) -> Fix {
    surround_with(token, params, "\"")
}

/// `checkSpacefulnessCfg' dirtyPass`, shared by `checkSpacefulnessCfg`
/// (`True`) and `checkVerboseSpacefulnessCfg` (`False`).
pub fn check_spacefulness_cfg_prime(dirty_pass: bool, params: &Params<'_>, token: Id, c: &mut Comments) {
    let Token::T_DollarBraced(_, list) = params.ast.get(token) else { return };
    let id = token;
    let braced_string = oversimplify(params.ast, *list).concat();
    let name = get_braced_reference(&braced_string);

    let needs_quoting = !is_array_expansion(params.ast, token) // There's another warning for this
        && !is_counting_reference(params.ast, token)
        && !is_quote_free(params.ast, params.shell_type, token)
        && !is_quoted_alternative_reference(params.ast, token)
        && !used_as_command_name(params.ast, token);

    let is_clean_state = |state: &VariableState| {
        state.variable_properties.all_have(CFVariableProp::CFVPInteger)
            || state.variable_value.space_status == SpaceStatus::SpaceStatusClean
    };

    let is_clean = (|| {
        let cfga = params.cfg_analysis.as_deref()?;
        let state = get_incoming_state(cfga, id)?;
        let value = state.variable_in_scope(&name)?;
        Some(is_clean_state(&value))
    })()
    .unwrap_or(false);

    let is_default_assignment = |token: Id| {
        let modifier = get_braced_modifier(&braced_string);
        ["=", ":="].iter().any(|p| modifier.starts_with(p)) && is_param_to(params.ast, ":", token)
    };

    if needs_quoting && (dirty_pass == !is_clean) {
        if !(SPECIAL_VARIABLES_WITHOUT_SPACES.contains(&name.as_str())
            || quotes_may_conflict_with_sc2281(params, token))
        {
            if dirty_pass {
                if is_default_assignment(token) {
                    c.info(token, 2223, "This default assignment may cause DoS due to globbing. Quote it.");
                } else {
                    c.info_with_fix(
                        id,
                        2086,
                        "Double quote to prevent globbing and word splitting.",
                        add_double_quotes_around(params, token),
                    );
                }
            } else {
                c.style_with_fix(
                    id,
                    2248,
                    "Prefer double quoting even when variables don't contain special characters.",
                    add_double_quotes_around(params, token),
                );
            }
        }
    }
}

/// Is `t` inside a `source`d file?
pub fn is_sourced(params: &Params<'_>, t: Id) -> bool {
    get_path(params.ast, t).iter().any(|&x| matches!(params.ast.get(x), Token::T_SourceCommand(..)))
}

// ---- Checks/Commands.hs ----

/// Catches occurrences of "grep -q" and variants, such as "-m" or "-L",
/// inside of pipes under pipefail. `cmd` is the grep `T_SimpleCommand`.
pub fn check_grep_sends_pipefail_impl(params: &Params<'_>, cmd: Id, c: &mut Comments) {
    // Contains "L", even though BSD grep does not exit early with this flag,
    // but GNU grep does. This is consistent with the linter practice of warning
    // about potential problems, while also allowing users to disable specific
    // linter checks locally.
    const EARLY_EXIT_FLAGS: &[&str] = &["q", "quiet", "--silent", "m", "max-count", "L"];
    const LONG_OPTIONS_CONSUMING_PARAMETER: &[&str] =
        &["regexp", "file", "max-count", "after-context", "before-context", "context", "directories", "devices"];
    let is_early_exit_flag = |name: &str| EARLY_EXIT_FLAGS.contains(&name);
    let parse_grep_opts = |args: &[Id]| {
        let longopts: Vec<(String, bool)> =
            LONG_OPTIONS_CONSUMING_PARAMETER.iter().map(|name| (name.to_string(), true)).collect();
        get_opts(params.ast, (true, true), "cilLnoqsvwxhHrRbaEFGPe:f:m:A:B:C:d:D:", &longopts, args)
    };
    let warn_msg = [
        "In pipefail mode, flags like -q, -m, or -L can cause grep to exit early, aborting the pipeline with SIGPIPE.",
        "Use a non-pipe input like '< <(cmd)' or '<<<' instead.",
    ]
    .join(" ");

    let pipefail = params.has_pipefail;
    let ast_path = get_path(params.ast, cmd);
    let fires = (|| {
        if !pipefail {
            return None;
        }
        let opts: Vec<String> = parse_grep_opts(&arguments(params.ast, cmd))?.into_iter().map(|o| o.0).collect();
        if !opts.iter().any(|o| is_early_exit_flag(o)) {
            return None;
        }
        let [_simple_cmd, grep_redirecting_cmd, parent_nodes @ ..] = &ast_path[..] else { return None };
        let Token::T_Pipeline(_, cmds) = params.ast.get(*parent_nodes.first()?) else { return None };
        let [_first, redirecting_cmds @ ..] = &cmds[..] else { return None };
        redirecting_cmds.contains(grep_redirecting_cmd).then_some(())
    })();
    if fires.is_some() {
        c.warn(cmd, 2337, warn_msg);
    }
}

/// `returnOrExit multi invalid`, applied to a `return`/`exit` command `t`:
/// `multi` gets the second of several arguments' first, `invalid` a single
/// argument that is not a status 0-255.
pub fn return_or_exit(
    multi: impl FnOnce(Id, &mut Comments),
    invalid: impl FnOnce(Id, &mut Comments),
    ast: &Ast,
    t: Id,
    c: &mut Comments,
) {
    let is_invalid = |s: &str| {
        // Haskell `isDigit` is ASCII 0-9; `length s > 5` bounds the `read`.
        s.is_empty()
            || s.chars().any(|ch| !ch.is_ascii_digit())
            || s.chars().count() > 5
            || s.parse::<u32>().is_ok_and(|value| value > 255)
    };
    let lit = |x: Id| -> Option<String> {
        Some(
            match ast.get(x) {
                Token::T_DollarBraced(..)
                | Token::T_DollarArithmetic(..)
                | Token::T_DollarExpansion(..)
                | Token::T_Backticked(..) => "0",
                _ => "WTF",
            }
            .to_string(),
        )
    };
    let literal = |token: Id| get_literal_string_ext(ast, &lit, token).expect("lit always succeeds");
    match &arguments(ast, t)[..] {
        [first, _second, ..] => multi(*first, c),
        [value] => {
            if is_invalid(&literal(*value)) {
                invalid(*value, c);
            }
        }
        _ => {}
    }
}

/// Return the single variable expansion that makes up this word, if any.
/// e.g. $foo -> $foo, "$foo"'' -> $foo , "hello $name" -> Nothing
pub fn get_single_unmodified_braced_string(ast: &Ast, word: Id) -> Option<String> {
    match &get_word_parts(ast, word)[..] {
        [part] => match ast.get(*part) {
            Token::T_DollarBraced(_, l) => {
                let contents = oversimplify(ast, *l).concat();
                let name = get_braced_reference(&contents);
                (contents == name).then_some(contents)
            }
            _ => None,
        },
        _ => None,
    }
}

/// `missingDestination handler token`: call `handler token` when the
/// command `token` (`mv`, `cp`, `ln`) has a single non-flag argument and no
/// target directory flag.
pub fn missing_destination(handler: impl FnOnce(Id, &mut Comments), ast: &Ast, token: Id, c: &mut Comments) {
    let args = get_all_flags(ast, token);
    let params: Vec<Id> = args.iter().filter(|(_, f)| f.is_empty()).map(|(x, _)| *x).collect();
    let has_target = args.iter().any(|(_, x)| !x.is_empty() && "target-directory".starts_with(x.as_str()));
    if let [single] = params[..]
        && !(has_target || may_become_multiple_args(ast, single))
    {
        handler(token, c);
    }
}

// ---- Synthetic tokens ----

/// A private copy of the arena for checks that build tokens the parser did
/// not, e.g. `braceExpand` results or `T_SimpleCommand id cmdPrefix rest`.
///
/// Haskell builds such tokens freely and usually reuses an existing id
/// (`T_NormalWord id items`). The shared `&Ast` is read-only and cannot
/// alias ids, so a check clones it into a `Scratch`, allocates each
/// synthetic token as a fresh node, and records which existing id the
/// Haskell token would carry. Before writing a comment about a synthetic
/// token, map its id back with `haskell_id`.
///
/// Synthetic nodes have no recorded parent; use `parent` here (which
/// answers for the Haskell id) instead of `ast.parent`.
///
/// Cloning is O(size of the script): only build a `Scratch` once a check
/// knows it needs one, and prefer computing on the parts directly when the
/// synthetic token is only inspected (see docs/porting-a-check.md).
pub struct Scratch {
    pub ast: Ast,
    origin: HashMap<Id, Id>,
}

impl Scratch {
    pub fn new(ast: &Ast) -> Scratch {
        Scratch { ast: ast.clone(), origin: HashMap::new() }
    }

    /// Allocate `t` standing for the Haskell token that reuses `like`'s id
    /// (and its span).
    pub fn synth(&mut self, like: Id, t: Token) -> Id {
        let (start, end) = self.ast.span(like);
        let id = self.ast.alloc(start, end);
        self.ast.set(id, t);
        self.origin.insert(id, self.haskell_id(like));
        id
    }

    /// `braceExpand` on word `id`; each result stands for `id`.
    pub fn brace_expand(&mut self, id: Id) -> Vec<Id> {
        let out = brace_expand(&mut self.ast, id);
        let h = self.haskell_id(id);
        for &x in &out {
            self.origin.insert(x, h);
        }
        out
    }

    /// The id the Haskell token has: the original id for synthetic nodes,
    /// `id` itself otherwise.
    pub fn haskell_id(&self, id: Id) -> Id {
        self.origin.get(&id).copied().unwrap_or(id)
    }

    /// The parent as Haskell's `parentMap` would answer for this token.
    pub fn parent(&self, id: Id) -> Option<Id> {
        self.ast.parent(self.haskell_id(id))
    }

    /// `params` with `ast` replaced by this copy.
    pub fn params<'s>(&'s self, p: &Params<'_>) -> Params<'s> {
        Params { ast: &self.ast, ..p.clone() }
    }
}

// ---- Registration ----

/// Register a parametrised command check for each name, as a Haskell
/// `commandChecks` entry `map f [...]` does: one `CheckEntry` per name,
/// named `<name> "<cmd>"` (e.g. `checkMaskedReturns "local"`), matching the
/// command with `CommandName::<kind>(cmd)`, all at `order`. `check` is
/// called as `check(cmd, params, t, comments)`.
///
/// The entries go in a `pub(crate) static ENTRIES: [CheckEntry; N]`, in list
/// order; `build.rs` adds `&ENTRIES[0]`, ..., `&ENTRIES[N - 1]` to
/// `crate::CHECKS`, so a file may invoke this at most once.
///
/// ```ignore
/// parametrised_command_checks!("checkArgComparison", 45, Exactly, check_arg_comparison,
///     ["alias", "local", "declare"]);
/// ```
macro_rules! parametrised_command_checks {
    ($name:literal, $order:literal, $kind:ident, $check:path, [$($cmd:literal),+ $(,)?]) => {
        pub(crate) static ENTRIES: [::shellcheck_analyzer::api::CheckEntry; [$($cmd),+].len()] = [
            $(
                ::shellcheck_analyzer::api::CheckEntry {
                    module: "Checks/Commands.hs",
                    name: concat!($name, " \"", $cmd, "\""),
                    registry: "command",
                    order: $order,
                    optional: None,
                    kind: ::shellcheck_analyzer::api::CheckKind::Command(
                        ::shellcheck_analyzer::api::CommandName::$kind($cmd),
                        {
                            fn run(
                                p: &::shellcheck_analyzer::analyzerlib::Params<'_>,
                                t: ::shellcheck_core::ast::Id,
                                c: &mut ::shellcheck_analyzer::analyzerlib::Comments,
                            ) {
                                $check($cmd, p, t, c)
                            }
                            run
                        },
                    ),
                },
            )+
        ];
    };
}
pub(crate) use parametrised_command_checks;
