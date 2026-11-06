// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkBraceExpansionVars`, lines 619-642 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command_m, is_unqualified_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string_ext;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkBraceExpansionVars",
    registry: "shell-support",
    order: 3,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash], f),
};

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_BraceExpansion(list) = p.ast.get(t) else { return };
    for &element in list {
        let s = to_string(p, element);
        if ["$..", "..$"].iter().any(|n| s.contains(n)) {
            if is_evaled(p, element) {
                c.style(t, 2175, "Quote this invalid brace expansion since it should be passed literally to eval.");
            } else {
                c.warn(t, 2051, "Bash doesn't support variables in brace range expansions.");
            }
        }
    }
}

fn literal_ext(p: &Params<'_>, t: Id) -> Option<String> {
    match p.ast.get(t) {
        Token::T_DollarBraced(..) | Token::T_DollarExpansion(..) | Token::T_DollarArithmetic(..) => Some("$".to_string()),
        _ => Some("-".to_string()),
    }
}

fn to_string(p: &Params<'_>, t: Id) -> String {
    get_literal_string_ext(p.ast, &|x| literal_ext(p, x), t).unwrap_or_default()
}

fn is_evaled(p: &Params<'_>, t: Id) -> bool {
    let cmd = get_closest_command_m(p, t);
    cmd.is_some_and(|c| is_unqualified_command(p.ast, c, "eval"))
}
