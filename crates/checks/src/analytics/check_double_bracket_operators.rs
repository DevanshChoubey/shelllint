// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDoubleBracketOperators`, lines 1395-1399 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDoubleBracketOperators",
    registry: "node",
    order: 10,
    optional: None,
    kind: CheckKind::Node(check_double_bracket_operators),
};

fn check_double_bracket_operators(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Binary(ConditionType::DoubleBracket, op, _, _) = p.ast.get(t) {
        if op == "\\<" || op == "\\>" {
            c.err(
                t,
                2075,
                &format!("Escaping {} is required in [..], but invalid in [[..]]", op),
            );
        }
    }
}
