// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkArgComparison`, lines 1363-1392 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    arguments, get_leading_unquoted_string, get_literal_string, get_trailing_unquoted_literal,
};

use crate::support::parametrised_command_checks;

parametrised_command_checks!(
    "checkArgComparison",
    45,
    Exactly,
    check_arg_comparison,
    ["alias", "local", "declare", "export", "readonly", "typeset", "let"]
);

fn check_arg_comparison(cmd: &str, p: &Params<'_>, t: Id, c: &mut Comments) {
    for arg in arguments(p.ast, t) {
        check(cmd, p, arg, c);
    }
}

fn check(cmd: &str, p: &Params<'_>, arg: Id, c: &mut Comments) {
    if let Some(s) = get_leading_unquoted_string(p.ast, arg) {
        if s.starts_with('=') {
            c.err(head_id(p, arg), 2290, "Remove spaces around = to assign.");
        } else if s.starts_with("+=") {
            c.err(head_id(p, arg), 2290, "Remove spaces around += to append.");
        }
    }

    // 'let' is parsed as a sequence of arithmetic expansions,
    // so we want the additional warning for "x="
    if cmd == "let" {
        if let Some(token) = get_trailing_unquoted_literal(p.ast, arg) {
            if let Some(s) = get_literal_string(p.ast, token) {
                if s.ends_with('=') {
                    c.err(token, 2290, "Remove spaces around = to assign.");
                }
            }
        }
    }
}

fn head_id(p: &Params<'_>, t: Id) -> Id {
    match p.ast.get(t) {
        Token::T_NormalWord(l) if !l.is_empty() => l[0],
        _ => t,
    }
}
