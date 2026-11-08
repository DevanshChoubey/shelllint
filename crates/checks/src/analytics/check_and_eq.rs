// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkAndEq`, lines 1733-1770 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{ConditionType, Id, Token};
use shelllint_core::astlib::{is_constant, is_literal, is_literal_number};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkAndEq",
    registry: "node",
    order: 20,
    optional: None,
    kind: CheckKind::Node(check_and_eq),
};

fn check_and_eq_operands(p: &Params<'_>, op: &str, rhs1: Id, rhs2: Id) -> bool {
    if op == "-eq" {
        return is_literal_number(p.ast, rhs1) && is_literal_number(p.ast, rhs2);
    }
    if op == "=" || op == "==" {
        return is_literal(p.ast, rhs1) && is_literal(p.ast, rhs2);
    }
    false
}

fn check_and_eq(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    match ast.get(t) {
        // For test-level "and": [ x = y -a x = z ]
        Token::TC_And(typ, _op, l, r) => {
            let (Token::TC_Binary(_, op1, lhs1, rhs1), Token::TC_Binary(_, op2, lhs2, rhs2)) =
                (ast.get(*l), ast.get(*r))
            else {
                return;
            };
            if op1 == op2
                && ast.structurally_eq(*lhs1, *lhs2)
                && !ast.structurally_eq(*rhs1, *rhs2)
                && check_and_eq_operands(p, op1, *rhs1, *rhs2)
            {
                c.warn(
                    t,
                    2333,
                    format!(
                        "You probably wanted {} here, otherwise it's always false.",
                        if *typ == ConditionType::SingleBracket { "-o" } else { "||" }
                    ),
                );
            }
        }
        // For arithmetic context "and"
        Token::TA_Binary(op, l, r) if op == "&&" => {
            let (Token::TA_Binary(o1, lhs1, rhs1), Token::TA_Binary(o2, lhs2, rhs2)) =
                (ast.get(*l), ast.get(*r))
            else {
                return;
            };
            if o1 == "==" && o2 == "=="
                && ast.structurally_eq(*lhs1, *lhs2)
                && is_literal_number(ast, *rhs1)
                && is_literal_number(ast, *rhs2)
            {
                c.warn(t, 2334, "You probably wanted || here, otherwise it's always false.");
            }
        }
        // For command level "and": [ x = y ] && [ x = z ]
        Token::T_AndIf(l, r) => {
            let Some((lhs1, op1, rhs1)) = get_expr(p, *l) else { return };
            let Some((lhs2, op2, rhs2)) = get_expr(p, *r) else { return };
            if op1 != op2 {
                return;
            }
            if !(ast.structurally_eq(lhs1, lhs2) && !ast.structurally_eq(rhs1, rhs2)) {
                return;
            }
            if !check_and_eq_operands(p, &op1, rhs1, rhs2) {
                return;
            }
            c.warn(t, 2333, "You probably wanted || here, otherwise it's always false.");
        }
        _ => {}
    }
}

fn get_expr(p: &Params<'_>, x: Id) -> Option<(Id, String, Id)> {
    match p.ast.get(x) {
        // Fetches x and y in `T_AndIf x (T_AndIf y z)`
        Token::T_AndIf(lhs, _) => get_expr(p, *lhs),
        Token::T_Pipeline(_, cmds) => match &cmds[..] {
            [only] => get_expr(p, *only),
            _ => None,
        },
        Token::T_Redirecting(_, cmd) => get_expr(p, *cmd),
        Token::T_Condition(_, _, cond) => get_expr(p, *cond),
        Token::TC_Binary(_, op, lhs, rhs) => orient(p, *lhs, op, *rhs),
        _ => None,
    }
}

// Swap items so that the constant side is rhs (or None if both/neither is constant)
fn orient(p: &Params<'_>, lhs: Id, op: &str, rhs: Id) -> Option<(Id, String, Id)> {
    match (is_constant(p.ast, lhs), is_constant(p.ast, rhs)) {
        (true, false) => Some((rhs, op.to_string(), lhs)),
        (false, true) => Some((lhs, op.to_string(), rhs)),
        _ => None,
    }
}
