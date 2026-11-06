// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkShorthandIf`, lines 932-943 (e11f964)

use crate::support::is_condition;
use shellcheck_analyzer::analyzerlib::{Comments, Params, is_test_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_command_basename, get_path, is_assignment};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkShorthandIf",
    registry: "node",
    order: 3,
    optional: None,
    kind: CheckKind::Node(check_shorthand_if),
};

fn check_shorthand_if(p: &Params<'_>, x: Id, c: &mut Comments) {
    let Token::T_OrIf(l, r) = p.ast.get(x) else { return };
    let Token::T_AndIf(_, b) = p.ast.get(*l) else { return };
    let id = *l;
    let Token::T_Pipeline(_, t) = p.ast.get(*r) else { return };
    let in_condition = is_condition(p.ast, &get_path(p.ast, x));
    if !(is_ok(p, t) || in_condition) && !is_test_command(p.ast, *b) {
        c.info(id, 2015, "Note that A && B || C is not if-then-else. C may run when A is true.");
    }
}

fn is_ok(p: &Params<'_>, ts: &[Id]) -> bool {
    let &[t] = ts else { return false };
    is_assignment(p.ast, t)
        || match get_command_basename(p.ast, t) {
            Some(name) => ["echo", "exit", "return", "printf", "true", ":"].contains(&name.as_str()),
            None => false,
        }
}
