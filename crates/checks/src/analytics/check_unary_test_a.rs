// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnaryTestA`, lines 5446-5452 (e11f964)

use crate::support::{fix_with, replace_start};
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnaryTestA",
    registry: "node",
    order: 102,
    optional: None,
    kind: CheckKind::Node(check_unary_test_a),
};

fn check_unary_test_a(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Unary(_, op, _) = p.ast.get(t) {
        if op == "-a" {
            c.style_with_fix(
                t,
                2331,
                "For file existence, prefer standard -e over legacy -a.",
                fix_with(vec![replace_start(t, p, 2, "-e")]),
            );
        }
    }
}
