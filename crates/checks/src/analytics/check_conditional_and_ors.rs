// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkConditionalAndOrs`, lines 1405-1422 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{ConditionType, Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkConditionalAndOrs",
    registry: "node",
    order: 53,
    optional: None,
    kind: CheckKind::Node(check_conditional_and_ors),
};

fn check_conditional_and_ors(p: &Params<'_>, t: Id, c: &mut Comments) {
    use ConditionType::{DoubleBracket, SingleBracket};
    match p.ast.get(t) {
        Token::TC_And(SingleBracket, op, _, _) if op == "&&" => {
            c.err(t, 2107, "Instead of [ a && b ], use [ a ] && [ b ].")
        }
        Token::TC_And(DoubleBracket, op, _, _) if op == "-a" => {
            c.err(t, 2108, "In [[..]], use && instead of -a.")
        }
        Token::TC_Or(SingleBracket, op, _, _) if op == "||" => {
            c.err(t, 2109, "Instead of [ a || b ], use [ a ] || [ b ].")
        }
        Token::TC_Or(DoubleBracket, op, _, _) if op == "-o" => {
            c.err(t, 2110, "In [[..]], use || instead of -o.")
        }
        Token::TC_And(SingleBracket, op, _, _) if op == "-a" => {
            c.warn(t, 2166, "Prefer [ p ] && [ q ] as [ p -a q ] is not well defined.")
        }
        Token::TC_Or(SingleBracket, op, _, _) if op == "-o" => {
            c.warn(t, 2166, "Prefer [ p ] || [ q ] as [ p -o q ] is not well defined.")
        }
        _ => {}
    }
}
