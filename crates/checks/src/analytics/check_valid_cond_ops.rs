// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkValidCondOps`, lines 1776-1783 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::data::{BINARY_TEST_OPS, UNARY_TEST_OPS};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkValidCondOps",
    registry: "node",
    order: 29,
    optional: None,
    kind: CheckKind::Node(check_valid_cond_ops),
};

fn check_valid_cond_ops(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::TC_Binary(_, s, _, _) if !BINARY_TEST_OPS.contains(&s.as_str()) => {
            c.warn(t, 2057, "Unknown binary operator.");
        }
        Token::TC_Unary(_, s, _) if !UNARY_TEST_OPS.contains(&s.as_str()) => {
            c.warn(t, 2058, "Unknown unary operator.");
        }
        _ => {}
    }
}
