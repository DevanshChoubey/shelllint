// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDivBeforeMult`, lines 1553-1557 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

use crate::support::has_floating_point;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDivBeforeMult",
    registry: "node",
    order: 13,
    optional: None,
    kind: CheckKind::Node(check_div_before_mult),
};

fn check_div_before_mult(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TA_Binary(op, l, y) = params.ast.get(t) else { return };
    if op != "*" {
        return;
    }
    let Token::TA_Binary(op2, _, x) = params.ast.get(*l) else { return };
    if op2 != "/" {
        return;
    }
    if !has_floating_point(params) && !params.ast.structurally_eq(*x, *y) {
        c.info(*l, 2017, "Increase precision by replacing a/b*c with a*c/b.");
    }
}
