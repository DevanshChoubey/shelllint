// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUselessBang`, lines 4326-4363 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_path;

use crate::support::{fix_with, is_condition, replace_end, replace_start};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUselessBang",
    registry: "node",
    order: 83,
    optional: None,
    kind: CheckKind::Node(check_useless_bang),
};

fn check_useless_bang(params: &Params<'_>, t: Id, c: &mut Comments) {
    if params.has_errexit {
        for x in get_non_returning_commands(params, t) {
            check(params, x, c);
        }
    }
}

fn check(params: &Params<'_>, t: Id, c: &mut Comments) {
    match *params.ast.get(t) {
        Token::T_Banged(cmd) if !is_condition(params.ast, &get_path(params.ast, t)) => {
            c.info_with_fix(
                t,
                2251,
                "This ! is not on a condition and skips errexit. Use `&& exit 1` instead, or make sure $? is checked.",
                fix_with(vec![
                    replace_start(t, params, 1, ""),
                    replace_end(cmd, params, 0, " && exit 1"),
                ]),
            );
        }
        Token::T_Annotation(_, inner) => check(params, inner, c),
        _ => {}
    }
}

/// Get all the subcommands that aren't likely to be the return value
fn get_non_returning_commands(params: &Params<'_>, t: Id) -> Vec<Id> {
    match params.ast.get(t) {
        Token::T_Script(_, list) => drop_last(list),
        Token::T_BraceGroup(list) => {
            if is_function_body(params, t) { drop_last(list) } else { list.clone() }
        }
        Token::T_Subshell(list) => drop_last(list),
        Token::T_WhileExpression(conds, cmds) | Token::T_UntilExpression(conds, cmds) => {
            let mut v = drop_last(conds);
            v.extend(cmds.iter().copied());
            v
        }
        Token::T_ForIn(_, _, list) => list.clone(),
        Token::T_ForArithmetic(_, _, _, list) => list.clone(),
        &Token::T_Annotation(_, inner) => get_non_returning_commands(params, inner),
        Token::T_IfExpression(conds, elses) => {
            let mut v: Vec<Id> = conds.iter().flat_map(|(cs, _)| drop_last(cs)).collect();
            v.extend(conds.iter().flat_map(|(_, body)| body.iter().copied()));
            v.extend(elses.iter().copied());
            v
        }
        _ => vec![],
    }
}

fn is_function_body(params: &Params<'_>, t: Id) -> bool {
    let path = get_path(params.ast, t);
    matches!(path.get(1), Some(&p) if matches!(params.ast.get(p), Token::T_Function(..)))
}

/// `dropLast t = [x | (x, _) <- zip t (tail t)]`: all but the last element.
fn drop_last(t: &[Id]) -> Vec<Id> {
    t[..t.len().saturating_sub(1)].to_vec()
}
