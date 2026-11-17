// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Builtins and their flags: SC9100 printf -v, 9102 declare -A, 9103 mapfile, 9111 globstar, 9112 read -t 0.5,
// 9114 read -N, 9115 printf -v a[i], 9116 declare -g, 9119 lastpipe, 9120 printf %(fmt)T, 9122 declare -n,
// 9124 wait -n, 9126 local -, 9127 mapfile -d, 9128 inherit_errexit, 9130 wait -f, 9131 localvar_inherit/unset,
// 9133 wait -p, 9136 patsub_replacement.
// Runs once per node, only when a bash target is set. Reports through `super::too_new`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_command_name_and_token, get_literal_string, oversimplify};

use super::{too_new, too_new_as};

/// One option letter of a builtin, the word it was found in and its argument (when the option takes one).
struct Opt {
    word: Id,
    flag: char,
    arg: Option<(Id, String)>,
}

fn text(p: &Params<'_>, w: Id) -> String {
    oversimplify(p.ast, w).concat()
}

/// Leading `-x` options of a builtin, as a GNU getopt would read them: clusters like `-rt 5` and `-t5` are
/// split, letters in `with_arg` take the rest of the cluster or the next word, and `--` ends the options.
/// Returns the options and the words that follow them. Stops at the first non-option or non-literal word.
fn options<'a>(p: &Params<'_>, args: &'a [Id], with_arg: &str) -> (Vec<Opt>, &'a [Id]) {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let Some(s) = get_literal_string(p.ast, args[i]) else { break };
        if s == "--" {
            i += 1;
            break;
        }
        let Some(cluster) = s.strip_prefix('-').filter(|c| !c.is_empty()) else { break };
        let word = args[i];
        i += 1;
        for (at, flag) in cluster.char_indices() {
            if !with_arg.contains(flag) {
                out.push(Opt { word, flag, arg: None });
                continue;
            }
            let attached = &cluster[at + flag.len_utf8()..];
            let arg = if !attached.is_empty() {
                Some((word, attached.to_string()))
            } else if i < args.len() {
                i += 1;
                Some((args[i - 1], text(p, args[i - 1])))
            } else {
                None
            };
            out.push(Opt { word, flag, arg });
            break;
        }
    }
    (out, &args[i..])
}

fn find(opts: &[Opt], flag: char) -> Option<&Opt> {
    opts.iter().find(|o| o.flag == flag)
}

/// Does a printf format use `%(fmt)T`? `%%` is a literal percent; flags and a width may precede the `(`.
fn has_time_format(fmt: &str) -> bool {
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            continue;
        }
        while chars.next_if(|c| "-+ #0123456789.".contains(*c)).is_some() {}
        if chars.next() == Some('(') {
            return true;
        }
    }
    false
}

/// A `-t` timeout with a fractional part.
fn is_fractional(s: &str) -> bool {
    s.contains('.') && s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.chars().any(|c| c.is_ascii_digit())
}

pub fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let (Some(name), tok) = get_command_name_and_token(p.ast, false, t) else { return };
    let Some(at) = words.iter().position(|&w| w == tok) else { return };
    let args = &words[at + 1..];
    match name.as_str() {
        "printf" => printf(p, c, t, args),
        "declare" | "typeset" | "local" => declaration(p, c, &name, args),
        "mapfile" | "readarray" => {
            too_new(p, c, t, 9103);
            let (opts, _) = options(p, args, "dnOsCc");
            if let Some(o) = find(&opts, 'd') {
                too_new(p, c, o.word, 9127);
            }
        }
        "read" => {
            let (opts, _) = options(p, args, "adinNptu");
            if let Some(o) = find(&opts, 'N') {
                too_new(p, c, o.word, 9114);
            }
            if let Some((w, s)) = find(&opts, 't').and_then(|o| o.arg.as_ref()) {
                if is_fractional(s) {
                    too_new(p, c, *w, 9112);
                }
            }
        }
        "wait" => {
            let (opts, _) = options(p, args, "p");
            for (flag, code) in [('n', 9124), ('f', 9130), ('p', 9133)] {
                if let Some(o) = find(&opts, flag) {
                    too_new(p, c, o.word, code);
                }
            }
        }
        "shopt" => shopt(p, c, args),
        _ => {}
    }
}

fn printf(p: &Params<'_>, c: &mut Comments, _t: Id, args: &[Id]) {
    let (opts, rest) = options(p, args, "v");
    if let Some(o) = find(&opts, 'v') {
        too_new(p, c, o.word, 9100);
        if let Some((w, var)) = &o.arg {
            if var.contains('[') {
                too_new(p, c, *w, 9115);
            }
        }
    }
    if let Some(&fmt) = rest.first() {
        if has_time_format(&text(p, fmt)) {
            too_new(p, c, fmt, 9120);
        }
    }
}

/// `declare`, `typeset` and `local`: attribute flags, and `local -` (which saves the shell options).
fn declaration(p: &Params<'_>, c: &mut Comments, name: &str, args: &[Id]) {
    let (opts, _) = options(p, args, "");
    let what = |flag: char| format!("{name} -{flag}");
    if let Some(o) = find(&opts, 'A') {
        too_new_as(p, c, o.word, 9102, Some(what('A').as_str()));
    }
    if let Some(o) = find(&opts, 'n') {
        too_new_as(p, c, o.word, 9122, Some(what('n').as_str()));
    }
    if name != "local" {
        if let Some(o) = find(&opts, 'g') {
            too_new_as(p, c, o.word, 9116, Some(what('g').as_str()));
        }
    }
    if name == "local" {
        if let Some(&w) = args.iter().find(|&&w| get_literal_string(p.ast, w).as_deref() == Some("-")) {
            too_new(p, c, w, 9126);
        }
    }
}

/// `shopt -s name...` for options newer than the target. Only enabling counts: `-u` and `-q` are fine.
fn shopt(p: &Params<'_>, c: &mut Comments, args: &[Id]) {
    let (opts, names) = options(p, args, "");
    if find(&opts, 's').is_none() || find(&opts, 'u').is_some() || find(&opts, 'o').is_some() {
        return;
    }
    for &w in names {
        let Some(code) = (match get_literal_string(p.ast, w).as_deref() {
            Some("globstar") => Some(9111),
            Some("lastpipe") => Some(9119),
            Some("inherit_errexit") => Some(9128),
            Some("localvar_inherit" | "localvar_unset") => Some(9131),
            Some("patsub_replacement") => Some(9136),
            _ => None,
        }) else {
            continue;
        };
        too_new(p, c, w, code);
    }
}
