// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkEqualsInCommand`, lines 4637-4785 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    get_braced_modifier, get_braced_reference, get_literal_string_ext, get_unquoted_literal, is_quoteable_expansion,
    is_quotes, is_variable_char, is_variable_name, oversimplify, will_become_multiple_args,
};
use shellcheck_core::interface::Shell;
use shellcheck_core::regex::{matches, mk_regex};

use crate::support::{fix_with, replace_end, replace_start, replace_token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkEqualsInCommand",
    registry: "node",
    order: 89,
    optional: None,
    kind: CheckKind::Node(check_equals_in_command),
};

static POSITIONAL_ASSIGNMENT_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^[0-9][0-9]?="));

fn check_equals_in_command(params: &Params<'_>, original_token: Id, c: &mut Comments) {
    let ast = params.ast;
    let Token::T_SimpleCommand(_, words) = ast.get(original_token) else { return };
    let Some(&word) = words.first() else { return };
    check(params, original_token, word, c);
}

fn has_equals(params: &Params<'_>, t: Id) -> bool {
    match params.ast.get(t) {
        Token::T_Literal(s) => s.contains('='),
        _ => false,
    }
}

fn check(params: &Params<'_>, original_token: Id, t: Id, c: &mut Comments) {
    let Token::T_NormalWord(list) = params.ast.get(t) else { return };
    if !list.iter().any(|&x| has_equals(params, x)) {
        return;
    }
    let Some(idx) = list.iter().position(|&x| has_equals(params, x)) else { return };
    let leading = strip_single_plus(params, &list[..idx]);
    msg(params, original_token, t, leading, list[idx], c);
}

// This is a workaround for the parser adding + and = as separate literals
fn strip_single_plus<'a>(params: &Params<'_>, l: &'a [Id]) -> &'a [Id] {
    if let Some((&last, rest)) = l.split_last() {
        if matches!(params.ast.get(last), Token::T_Literal(s) if s == "+") {
            return rest;
        }
    }
    l
}

fn is_conflict_marker(params: &Params<'_>, cmd: Id) -> bool {
    let Some(s) = get_unquoted_literal(params.ast, cmd) else { return false };
    if !s.chars().all(|c| c == '=') {
        return false;
    }
    let n = s.chars().count();
    // Git uses 7 but who knows
    n >= 4 && n <= 12
}

fn may_be_variable_name(params: &Params<'_>, l: &[Id]) -> bool {
    let ast = params.ast;
    if l.iter().any(|&x| is_quotes(ast, x)) {
        return false;
    }
    if l.iter().any(|&x| will_become_multiple_args(ast, x)) {
        return false;
    }
    // DIVERGENCE: the synthetic `T_NormalWord (Id 0) l` is not built; its literal
    // string is the concatenation of the literal strings of its parts.
    let mut str = String::new();
    for &x in l {
        match get_literal_string_ext(ast, &|_| Some("x".to_string()), x) {
            Some(s) => str.push_str(&s),
            None => return false,
        }
    }
    is_variable_name(&str)
}

fn is_leading_number_var(s: &str) -> bool {
    let lead: String = s.chars().take_while(|&c| c != '=').collect();
    match lead.chars().next() {
        Some(x) => x.is_ascii_digit() && lead.chars().all(is_variable_char) && !lead.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

fn assign0_msg(params: &Params<'_>, id: Id, bashfix: shellcheck_core::interface::Fix, c: &mut Comments) {
    match params.shell_type {
        Shell::Bash => c.err_with_fix(
            id,
            2277,
            "Use BASH_ARGV0 to assign to $0 in bash (or use [ ] to compare).",
            bashfix,
        ),
        Shell::Ksh => c.err(id, 2278, "$0 can't be assigned in Ksh (but it does reflect the current function)."),
        Shell::Dash => c.err(id, 2279, "$0 can't be assigned in Dash. This becomes a command name."),
        Shell::BusyboxSh => c.err(id, 2279, "$0 can't be assigned in Busybox Ash. This becomes a command name."),
        _ => c.err(id, 2280, "$0 can't be assigned this way, and there is no portable alternative."),
    }
}

fn msg(params: &Params<'_>, original_token: Id, cmd: Id, leading: &[Id], eq: Id, c: &mut Comments) {
    let ast = params.ast;
    let lit_id = eq;
    let Token::T_Literal(s) = ast.get(eq) else { return };
    let s: &str = s;

    let positional_msg = |c: &mut Comments, id: Id| {
        c.err(id, 2270, "To assign positional parameters, use 'set -- first second ..' (or use [ ] to compare).")
    };
    let indirection_msg = |c: &mut Comments, id: Id| {
        c.err(id, 2271, "For indirection, use arrays, declare \"var$n=value\", or (for sh) read/eval.")
    };
    let generic_msg = |c: &mut Comments, id: Id| {
        c.err(id, 2276, "This is interpreted as a command name containing '='. Bad assignment or comparison?")
    };

    // There are many different cases, and the order of the branches matter.

    // --foo=42
    if leading.is_empty() && s.starts_with('-') {
        // There's SC2215 for these
        return;
    }

    // ======Hello======
    if leading.is_empty() && s.starts_with('=') {
        let single_word = match ast.get(original_token) {
            Token::T_SimpleCommand(assigns, words) if assigns.is_empty() && words.len() == 1 => Some(words[0]),
            _ => None,
        };
        match single_word {
            Some(word) if is_conflict_marker(params, word) => {
                c.err(original_token, 2273, "Sequence of ===s found. Merge conflict or intended as a commented border?")
            }
            _ if s.starts_with("===") => {
                c.err(original_token, 2274, "Command name starts with ===. Intended as a commented border?")
            }
            _ => c.err(cmd, 2275, "Command name starts with =. Bad line break?"),
        }
        return;
    }

    // '$var==42'
    if s.contains("==") {
        c.err(cmd, 2272, "Command name contains ==. For comparison, use [ \"$var\" = value ].");
        return;
    }

    // '${foo[x]}=42' and '$foo=42'
    if leading.len() == 1 && s.starts_with('=') {
        if let Token::T_DollarBraced(braced, l) = ast.get(leading[0]) {
            let id = leading[0];
            let braced = *braced;
            let variable_str: String = oversimplify(ast, *l).concat();
            let variable_reference = get_braced_reference(&variable_str);
            let variable_modifier = get_braced_modifier(&variable_str);
            let is_plain = is_variable_name(&variable_str);
            let is_positional = variable_str.chars().all(|c| c.is_ascii_digit());

            let is_array = variable_reference != "" && variable_modifier.starts_with('[') && variable_modifier.ends_with(']');

            if variable_str.is_empty() {
                // Don't try to fix ${}=foo
                generic_msg(c, cmd);
            } else if variable_str.starts_with('#') {
                // '$#=42' or '${#var}=42'
                generic_msg(c, cmd);
            } else if variable_str == "0" {
                // '${0}=42'
                assign0_msg(params, id, fix_with(vec![replace_token(id, params, "BASH_ARGV0")]), c);
            } else if is_positional {
                // '$2=2'
                positional_msg(c, id);
            } else if is_array || is_plain {
                c.err_with_fix(
                    id,
                    2281,
                    format!("Don't use {} on the left side of assignments.", if braced { "${}" } else { "$" }),
                    fix_with(if braced {
                        vec![replace_start(id, params, 2, ""), replace_end(id, params, 1, "")]
                    } else {
                        vec![replace_start(id, params, 1, "")]
                    }),
                );
            } else {
                indirection_msg(c, id);
            }
            return;
        }
    }

    // 2=42
    if leading.is_empty() && matches(s, &POSITIONAL_ASSIGNMENT_RE) {
        if s.starts_with("0=") {
            assign0_msg(params, lit_id, fix_with(vec![replace_start(lit_id, params, 1, "BASH_ARGV0")]), c);
        } else {
            positional_msg(c, lit_id);
        }
        return;
    }

    // 9foo=42
    if leading.is_empty() && is_leading_number_var(s) {
        c.err(cmd, 2282, "Variable names can't start with numbers, so this is interpreted as a command.");
        return;
    }

    // var${foo}x=42
    if !leading.is_empty()
        && may_be_variable_name(params, leading)
        && s.chars().take_while(|&ch| ch != '=').all(is_variable_char)
    {
        indirection_msg(c, cmd);
        return;
    }

    generic_msg(c, cmd);
}

#[allow(dead_code)]
fn is_expansion(params: &Params<'_>, t: Id) -> bool {
    match params.ast.get(t) {
        Token::T_Arithmetic(..) => true,
        _ => is_quoteable_expansion(params.ast, t),
    }
}
