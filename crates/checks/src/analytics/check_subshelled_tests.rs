// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSubshelledTests`, lines 4090-4167 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, get_path, is_function, oversimplify};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSubshelledTests",
    registry: "node",
    order: 80,
    optional: None,
    kind: CheckKind::Node(check_subshelled_tests),
};

fn check_subshelled_tests(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    let Token::T_Subshell(list) = ast.get(t) else { return };
    if !(list.iter().all(|&x| is_test_structure(ast, x)) && !has_assignment(ast, t)) {
        return;
    }
    let path = get_path(ast, t);
    // Special case for if (test) and while (test)
    if is_compound_condition(ast, &path) {
        c.style(t, 2233, "Remove superfluous (..) around condition to avoid subshell overhead.");
    // Special case for ([ x ]), except for func() ( [ x ] )
    } else if is_single_test(ast, list) && !is_function_body(ast, &path) {
        c.style(t, 2234, "Remove superfluous (..) around test command to avoid subshell overhead.");
    // General case for ([ x ] || [ y ] && etc)
    } else {
        c.style(t, 2235, "Use { ..; } instead of (..) to avoid subshell overhead.");
    }
}

fn is_single_test(ast: &Ast, cmds: &[Id]) -> bool {
    matches!(cmds, [c] if is_test_command(ast, *c))
}

fn is_function_body(ast: &Ast, path: &[Id]) -> bool {
    match path {
        [_, f, ..] => is_function(ast, *f),
        _ => false,
    }
}

fn is_test_structure(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Banged(x) => is_test_structure(ast, *x),
        Token::T_AndIf(a, b) => is_test_structure(ast, *a) && is_test_structure(ast, *b),
        Token::T_OrIf(a, b) => is_test_structure(ast, *a) && is_test_structure(ast, *b),
        Token::T_Pipeline(l, cmds) if l.is_empty() && cmds.len() == 1 => {
            if let Token::T_Redirecting(_, cmd) = ast.get(cmds[0]) {
                match ast.get(*cmd) {
                    Token::T_BraceGroup(ts) => ts.iter().all(|&x| is_test_structure(ast, x)),
                    Token::T_Subshell(ts) => ts.iter().all(|&x| is_test_structure(ast, x)),
                    _ => is_test_command(ast, t),
                }
            } else {
                is_test_command(ast, t)
            }
        }
        _ => is_test_command(ast, t),
    }
}

fn is_test_command(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Pipeline(l, cmds) if l.is_empty() && cmds.len() == 1 => {
            if let Token::T_Redirecting(_, cmd) = ast.get(cmds[0]) {
                match ast.get(*cmd) {
                    Token::T_Condition(..) => true,
                    _ => is_command(ast, *cmd, "test"),
                }
            } else {
                false
            }
        }
        _ => false,
    }
}

// Check if a T_Subshell is used as a condition, e.g. if ( test )
// This technically also triggers for `if true; then ( test ); fi`
// but it's still a valid suggestion.
fn is_compound_condition(ast: &Ast, chain: &[Id]) -> bool {
    let rest = chain.get(1..).unwrap_or(&[]);
    match rest.iter().find(|&&x| !skippable(ast, x)) {
        Some(&x) => matches!(
            ast.get(x),
            Token::T_IfExpression(..) | Token::T_WhileExpression(..) | Token::T_UntilExpression(..)
        ),
        None => false,
    }
}

fn has_assignment(ast: &Ast, t: Id) -> bool {
    // doAnalysis guardNotAssignment t is Nothing if any node yields Nothing
    let mut found = false;
    ast.analyze(t, &mut |n| {
        if !guard_not_assignment(ast, n) {
            found = true;
        }
    }, &mut |_| {});
    found
}

/// False corresponds to Haskell's `Nothing`.
fn guard_not_assignment(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::TA_Assignment(..) => false,
        Token::TA_Unary(s, _) => !(s.contains("++") || s.contains("--")),
        Token::T_DollarBraced(_, l) => {
            let str = oversimplify(ast, *l).concat();
            let modifier = get_braced_modifier(&str);
            !(modifier.starts_with('=') || modifier.starts_with(":="))
        }
        Token::T_DollarBraceCommandExpansion(..) => false,
        _ => true,
    }
}

// Skip any parent of a T_Subshell until we reach something interesting
fn skippable(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Redirecting(r, _) => r.is_empty(),
        Token::T_Pipeline(l, _) => l.is_empty(),
        Token::T_Annotation(..) => true,
        _ => false,
    }
}
