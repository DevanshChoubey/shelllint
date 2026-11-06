// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkLoopVariableReassignment`, lines 3461-3485 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_path_m};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkLoopVariableReassignment",
    registry: "node",
    order: 67,
    optional: None,
    kind: CheckKind::Node(check_loop_variable_reassignment),
};

fn loop_variable(ast: &Ast, t: Id) -> Option<String> {
    match ast.get(t) {
        Token::T_ForIn(s, _, _) => Some(s.clone()),
        Token::T_ForArithmetic(init, _, _, _) => {
            let Token::TA_Sequence(items) = ast.get(*init) else { return None };
            let &[a] = &items[..] else { return None };
            let Token::TA_Assignment(op, lhs, _) = ast.get(a) else { return None };
            if op != "=" {
                return None;
            }
            let Token::TA_Variable(var, _) = ast.get(*lhs) else { return None };
            Some(var.clone())
        }
        _ => None,
    }
}

fn check_loop_variable_reassignment(p: &Params<'_>, t: Id, c: &mut Comments) {
    if !matches!(p.ast.get(t), Token::T_ForIn(..) | Token::T_ForArithmetic(..)) {
        return;
    }
    let Some(s) = loop_variable(p.ast, t) else { return };
    if s == "_" {
        return;
    }
    let path = get_path_m(p, t);
    let Some(&next) = path
        .iter()
        .skip(1)
        .find(|&&x| loop_variable(p.ast, x).as_deref() == Some(s.as_str()))
    else {
        return;
    };
    c.warn(t, 2165, "This nested loop overrides the index variable of its parent.");
    c.warn(next, 2167, "This parent loop has its index variable overridden.");
}
