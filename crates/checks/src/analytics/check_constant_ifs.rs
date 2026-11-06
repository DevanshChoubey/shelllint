// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkConstantIfs`, lines 1474-1488 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::{is_constant, words_can_be_equal};
use shellcheck_core::data::ARITHMETIC_BINARY_TEST_OPS;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkConstantIfs",
    registry: "node",
    order: 22,
    optional: None,
    kind: CheckKind::Node(check_constant_ifs),
};

fn check_constant_ifs(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Binary(typ, op, lhs, rhs) = p.ast.get(t) else { return };
    let (typ, lhs, rhs) = (*typ, *lhs, *rhs);
    let op = op.as_str();
    let is_dynamic = (ARITHMETIC_BINARY_TEST_OPS.contains(&op) && typ == ConditionType::DoubleBracket)
        || ["-nt", "-ot", "-ef"].contains(&op);
    if is_dynamic {
        return;
    }
    if is_constant(p.ast, lhs) && is_constant(p.ast, rhs) {
        c.warn(t, 2050, "This expression is constant. Did you forget the $ on a variable?");
    } else if ["=", "==", "!="].contains(&op) && !words_can_be_equal(p.ast, lhs, rhs) {
        c.warn(
            t,
            2193,
            "The arguments to this comparison can never be equal. Make sure your syntax is correct.",
        );
    }
}
