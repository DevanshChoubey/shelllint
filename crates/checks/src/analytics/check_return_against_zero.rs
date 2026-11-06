// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkReturnAgainstZero`, lines 3533-3599 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    get_literal_string, get_path, get_word_parts, is_function, oversimplify,
};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkReturnAgainstZero",
    registry: "node",
    order: 69,
    optional: None,
    kind: CheckKind::Node(check_return_against_zero),
};

fn check_return_against_zero(p: &Params<'_>, token: Id, c: &mut Comments) {
    let ast = p.ast;

    // We don't want to warn about composite expressions like
    // [[ $? -eq 0 || $? -eq 4 ]] since these can be annoying to rewrite.
    fn is_only_test_in_command(p: &Params<'_>, t: Id) -> bool {
        let ast = p.ast;
        let path = get_path(ast, t);
        let Some(&next) = path.get(1) else { return false };
        match ast.get(next) {
            Token::T_Condition(..) => true,
            Token::T_Arithmetic(..) => true,
            Token::TA_Sequence(l)
                if l.len() == 1
                    && path
                        .get(2)
                        .is_some_and(|&n| matches!(ast.get(n), Token::T_Arithmetic(..))) =>
            {
                true
            }

            // Some negations and groupings are also fine
            Token::TC_Unary(_, op, _) if op == "!" => is_only_test_in_command(p, next),
            Token::TA_Unary(op, _) if op == "!" => is_only_test_in_command(p, next),
            Token::TC_Group(..) => is_only_test_in_command(p, next),
            Token::TA_Sequence(l) if l.len() == 1 => is_only_test_in_command(p, next),
            Token::TA_Parenthesis(..) => is_only_test_in_command(p, next),
            _ => false,
        }
    }

    // TODO: Do better $? tracking and filter on whether
    // the target command is in the same function
    fn get_first_command_in_function(p: &Params<'_>, t: Id) -> Id {
        let ast = p.ast;
        match ast.get(t) {
            Token::T_Function(_, _, _, x) => get_first_command_in_function(p, *x),
            Token::T_BraceGroup(l) if !l.is_empty() => get_first_command_in_function(p, l[0]),
            Token::T_Subshell(l) if !l.is_empty() => get_first_command_in_function(p, l[0]),
            Token::T_Annotation(_, x) => get_first_command_in_function(p, *x),
            Token::T_AndIf(x, _) => get_first_command_in_function(p, *x),
            Token::T_OrIf(x, _) => get_first_command_in_function(p, *x),
            Token::T_Pipeline(_, l) if !l.is_empty() => get_first_command_in_function(p, l[0]),
            Token::T_Redirecting(_, inner) => {
                if let Token::T_IfExpression(conds, _) = ast.get(*inner) {
                    if let Some((cond, _)) = conds.first() {
                        if let Some(&x) = cond.first() {
                            return get_first_command_in_function(p, x);
                        }
                    }
                }
                t
            }
            _ => t,
        }
    }

    let is_first_command_in_function = || -> bool {
        let path = get_path(ast, token);
        let Some(&func) = path.iter().find(|&&x| is_function(ast, x)) else {
            return false;
        };
        let Some(cmd) = get_closest_command(ast, token) else {
            return false;
        };
        cmd == get_first_command_in_function(p, func)
    };

    // Is "$? op 0" trying to check if the command succeeded?
    let checks_success_lhs = |op: &str| !["-gt", "-ne", "!=", "!"].contains(&op);
    // Is "0 op $?" trying to check if the command succeeded?
    let checks_success_rhs = |op: &str| !["-ne", "!="].contains(&op);

    let is_zero = |t: Id| get_literal_string(ast, t).as_deref() == Some("0");
    let is_exit_code = |t: Id| -> bool {
        let parts = get_word_parts(ast, t);
        match &parts[..] {
            &[x] => match ast.get(x) {
                Token::T_DollarBraced(_, l) => oversimplify(ast, *l).concat() == "?",
                _ => false,
            },
            _ => false,
        }
    };

    let message = |for_success: bool, id: Id, c: &mut Comments| {
        if is_only_test_in_command(p, token) && !is_first_command_in_function() {
            c.style(
                id,
                2181,
                format!(
                    "Check exit code directly with e.g. 'if {}mycmd;', not indirectly with $?.",
                    if for_success { "" } else { "! " }
                ),
            );
        }
    };

    let check = |op: &str, lhs: Id, rhs: Id, c: &mut Comments| {
        if is_zero(rhs) && is_exit_code(lhs) {
            message(checks_success_lhs(op), lhs, c);
        } else if is_zero(lhs) && is_exit_code(rhs) {
            message(checks_success_rhs(op), rhs, c);
        }
    };

    match ast.get(token) {
        Token::TC_Binary(_, op, lhs, rhs) => check(op, *lhs, *rhs, c),
        Token::TA_Binary(op, lhs, rhs)
            if [">", "<", ">=", "<=", "==", "!="].contains(&op.as_str()) =>
        {
            check(op, *lhs, *rhs, c)
        }
        Token::TA_Unary(op, exp) if op == "!" && is_exit_code(*exp) => {
            message(checks_success_lhs(op), *exp, c)
        }
        Token::TA_Sequence(l) if l.len() == 1 && is_exit_code(l[0]) => message(false, l[0], c),
        _ => {}
    }
}
