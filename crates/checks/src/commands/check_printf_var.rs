// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkPrintfVar`, lines 746-785 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string, is_literal, may_become_multiple_args, oversimplify};
use shellcheck_core::regex::{match_regex, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkPrintfVar",
    registry: "command",
    order: 15,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("printf"), check_printf_var),
};

/// `f . arguments`
fn check_printf_var(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    f(p, &args, c);
}

fn f(p: &Params<'_>, args: &[Id], c: &mut Comments) {
    let mut args = args;
    loop {
        match args {
            [doubledash, rest @ ..] if get_literal_string(p.ast, *doubledash).as_deref() == Some("--") => {
                args = rest;
            }
            [dashv, _var, rest @ ..] if get_literal_string(p.ast, *dashv).as_deref() == Some("-v") => {
                args = rest;
            }
            [dashv_var, rest @ ..] if is_dash_v_attached(&get_literal_string(p.ast, *dashv_var)) => {
                args = rest;
            }
            [format, params @ ..] => {
                check(p, *format, params, c);
                return;
            }
            [] => return,
        }
    }
}

/// `Just ('-':'v':_:_) <- getLiteralString x`
fn is_dash_v_attached(s: &Option<String>) -> bool {
    match s {
        Some(s) => {
            let mut it = s.chars();
            it.next() == Some('-') && it.next() == Some('v') && it.next().is_some()
        }
        None => false,
    }
}

fn check(p: &Params<'_>, format: Id, more: &[Id], c: &mut Comments) {
    if let Some(string) = get_literal_string(p.ast, format) {
        let formats = get_printf_formats(&string);
        let format_count = formats.chars().count();
        let arg_count = more.len();
        let pluralise_if_many = |word: &str, n: usize| {
            if n == 1 { word.to_string() } else { format!("{word}s") }
        };

        if arg_count == 0 && format_count == 0 {
            // This is fine
        } else if format_count == 0 && arg_count > 0 {
            c.err(
                format,
                2182,
                "This printf format string has no variables. Other arguments are ignored.",
            );
        } else if more.iter().any(|m| may_become_multiple_args(p.ast, *m)) {
            // We don't know so trust the user
        } else if arg_count < format_count && only_trailing_ts(&formats, arg_count) {
            // Allow trailing %()Ts since they use the current time
        } else if arg_count > 0 && arg_count.is_multiple_of(format_count) {
            // Great: a suitable number of arguments
        } else {
            c.warn(
                format,
                2183,
                format!(
                    "This format string has {} {}, but is passed {}{}.",
                    format_count,
                    pluralise_if_many("variable", format_count),
                    arg_count,
                    pluralise_if_many(" argument", arg_count),
                ),
            );
        }
    }

    if !(oversimplify(p.ast, format).concat().contains('%') || is_literal(p.ast, format)) {
        c.info(
            format,
            2059,
            "Don't use variables in the printf format string. Use printf '..%s..' \"$foo\".",
        );
    }
}

fn only_trailing_ts(format: &str, arg_count: usize) -> bool {
    format.chars().skip(arg_count).all(|c| c == 'T')
}

/// Get the arguments in the string as a string of type characters,
/// e.g. "Hello %s" -> "s" and "%(%s)T %0*d\n" -> "T*d"
fn get_printf_formats(string: &str) -> String {
    get_formats(string)
}

fn get_formats(string: &str) -> String {
    let mut out = String::new();
    let mut s = string;
    loop {
        if let Some(rest) = s.strip_prefix("%%") {
            s = rest;
        } else if let Some(rest) = s.strip_prefix("%(") {
            let after = match rest.find(')') {
                Some(i) => &rest[i..],
                None => "",
            };
            let mut it = after.chars();
            if it.next() == Some(')')
                && let Some(ch) = it.next()
            {
                out.push(ch);
                s = it.as_str();
            } else {
                return out;
            }
        } else if let Some(rest) = s.strip_prefix('%') {
            match regex_based_get_formats(rest) {
                (add, Some(next)) => {
                    out.push_str(&add);
                    s = next;
                }
                (add, None) => {
                    out.push_str(&add);
                    return out;
                }
            }
        } else if let Some(ch) = s.chars().next() {
            s = &s[ch.len_utf8()..];
        } else {
            return out;
        }
    }
}

static RE: LazyLock<Regex> = LazyLock::new(|| {
    // constructed based on specifications in "man printf"
    // flags, field width, precision, length modifier, format character, rest
    // DIVERGENCE: `(\n|.)*` is written as `(?s:.*)` (same language, leftmost-first safe).
    mk_regex(r"^#?-?\+? ?0?(\*|\d*)\.?(\d*|\*)(hh|h|l|ll|q|L|j|z|Z|t)?([diouxXfFeEgGaAcsbqQSC])((?s:.*))")
});

/// Returns the characters to emit before continuing, and the remaining input to
/// continue with (None when there is nothing more to scan).
fn regex_based_get_formats(rest: &str) -> (String, Option<&str>) {
    match match_regex(&RE, rest) {
        Some(g) if g.len() >= 5 => {
            let (width, precision, typ, after) = (&g[0], &g[1], &g[3], &g[4]);
            let mut out = String::new();
            if width == "*" {
                out.push('*');
            }
            if precision == "*" {
                out.push('*');
            }
            out.push_str(typ);
            // `rest` is a suffix of the input, so the match tail is a suffix of it.
            let tail = &rest[rest.len() - after.len()..];
            (out, Some(tail))
        }
        _ => {
            // `take 1 rest ++ getFormats rest`
            let first: String = rest.chars().take(1).collect();
            (first, Some(rest))
        }
    }
}
