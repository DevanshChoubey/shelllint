// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkOrNeq`, lines 1675-1712 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{ConditionType, Id, Token};
use shelllint_core::astlib::{is_constant, is_glob};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkOrNeq",
    registry: "node",
    order: 19,
    optional: None,
    kind: CheckKind::Node(check_or_neq),
};

fn check_or_neq(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    match ast.get(t) {
        // For test-level "or": [ x != y -o x != z ]
        Token::TC_Or(typ, _, l, r) => {
            let (Token::TC_Binary(_, op1, lhs1, rhs1), Token::TC_Binary(_, op2, lhs2, rhs2)) =
                (ast.get(*l), ast.get(*r))
            else {
                return;
            };
            if (op1 == op2 && (op1 == "-ne" || op1 == "!="))
                && ast.structurally_eq(*lhs1, *lhs2)
                && !ast.structurally_eq(*rhs1, *rhs2)
                && !(is_glob(ast, *rhs1) || is_glob(ast, *rhs2))
            {
                c.warn(
                    t,
                    2055,
                    format!(
                        "You probably wanted {} here, otherwise it's always true.",
                        if *typ == ConditionType::SingleBracket { "-a" } else { "&&" }
                    ),
                );
            }
        }
        // For arithmetic context "or"
        Token::TA_Binary(op, l, r) if op == "||" => {
            let (Token::TA_Binary(o1, word1, _), Token::TA_Binary(o2, word2, _)) =
                (ast.get(*l), ast.get(*r))
            else {
                return;
            };
            if o1 == "!=" && o2 == "!=" && ast.structurally_eq(*word1, *word2) {
                c.warn(t, 2056, "You probably wanted && here, otherwise it's always true.");
            }
        }
        // For command level "or": [ x != y ] || [ x != z ]
        Token::T_OrIf(lhs, rhs) => {
            let Some((lhs1, op1, rhs1)) = get_expr(p, *lhs) else { return };
            let Some((lhs2, op2, rhs2)) = get_expr(p, *rhs) else { return };
            if !(op1 == op2 && (op1 == "-ne" || op1 == "!=")) {
                return;
            }
            if !(ast.structurally_eq(lhs1, lhs2) && !ast.structurally_eq(rhs1, rhs2)) {
                return;
            }
            if is_glob(ast, rhs1) || is_glob(ast, rhs2) {
                return;
            }
            c.warn(t, 2252, "You probably wanted && here, otherwise it's always true.");
        }
        _ => {}
    }
}

fn get_expr(p: &Params<'_>, x: Id) -> Option<(Id, String, Id)> {
    let ast = p.ast;
    match ast.get(x) {
        // Fetches x and y in `T_OrIf x (T_OrIf y z)`
        Token::T_OrIf(lhs, _) => get_expr(p, *lhs),
        Token::T_Pipeline(_, cmds) => {
            let &[x] = &cmds[..] else { return None };
            get_expr(p, x)
        }
        Token::T_Redirecting(_, cmd) => get_expr(p, *cmd),
        Token::T_Condition(_, _, cond) => get_expr(p, *cond),
        Token::TC_Binary(_, op, lhs, rhs) => orient(p, *lhs, op.clone(), *rhs),
        _ => None,
    }
}

// Swap items so that the constant side is rhs (or Nothing if both/neither is constant)
fn orient(p: &Params<'_>, lhs: Id, op: String, rhs: Id) -> Option<(Id, String, Id)> {
    match (is_constant(p.ast, lhs), is_constant(p.ast, rhs)) {
        (true, false) => Some((rhs, op, lhs)),
        (false, true) => Some((lhs, op, rhs)),
        _ => None,
    }
}
