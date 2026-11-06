// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnnecessarilyInvertedTest`, lines 4183-4239 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::get_literal_string;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnnecessarilyInvertedTest",
    registry: "optional-tree",
    order: 2,
    optional: Some(StaticCheckDescription {
        name: "avoid-negated-conditions",
        id: &[2335],
        description: "Suggest removing unnecessary comparison negations",
        positive: "[ ! \"$var\" -eq 1 ]",
        negative: "[ \"$var\" -ne 1 ]",
    }),
    kind: CheckKind::Node(check_unnecessarily_inverted_test),
};

/// `T_Banged _ (T_Pipeline _ _ [T_Redirecting _ _ inner])` -> inner
fn banged_single(p: &Params<'_>, t: Id) -> Option<Id> {
    let Token::T_Banged(pl) = p.ast.get(t) else { return None };
    let Token::T_Pipeline(_, cmds) = p.ast.get(*pl) else { return None };
    let &[r] = &cmds[..] else { return None };
    let Token::T_Redirecting(_, inner) = p.ast.get(r) else { return None };
    Some(*inner)
}

fn inversion(op: &str) -> Option<&'static str> {
    Some(match op {
        "=" => "!=",
        "==" => "!=",
        "!=" => "=",
        "-eq" => "-ne",
        "-ne" => "-eq",
        "-le" => "-gt",
        "-gt" => "-le",
        "-ge" => "-lt",
        "-lt" => "-ge",
        _ => return None,
    })
}

fn maybe_suggest_rewrite(c: &mut Comments, bang_inside: bool, bracket_style: ConditionType, id: Id, op: &str) {
    let Some(new_op) = inversion(op) else { return };
    let old_expr = format!("a {op} b");
    let new_expr = format!("a {new_op} b");
    let bracket = |s: &str| {
        if bracket_style == ConditionType::SingleBracket {
            format!("[ {s} ]")
        } else {
            format!("[[ {s} ]]")
        }
    };
    if bang_inside {
        c.style(id, 2335, format!("Use {new_expr} instead of ! {old_expr}."));
    } else {
        c.style(id, 2335, format!("Use {} instead of ! {}.", bracket(&new_expr), bracket(&old_expr)));
    }
}

fn check_unnecessarily_inverted_test(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    match ast.get(t) {
        Token::TC_Unary(_, bang, inner) if bang == "!" => {
            match ast.get(*inner) {
                Token::TC_Unary(_, op, _) => match op.as_str() {
                    "-n" => c.style(t, 2236, "Use -z instead of ! -n."),
                    "-z" => c.style(t, 2236, "Use -n instead of ! -z."),
                    _ => {}
                },
                Token::TC_Binary(style, op, _, _) => maybe_suggest_rewrite(c, true, *style, t, op),
                _ => {}
            }
        }
        Token::T_Banged(_) => {
            let Some(inner) = banged_single(p, t) else { return };
            match ast.get(inner) {
                Token::T_Condition(_, _, cond) => match ast.get(*cond) {
                    Token::TC_Unary(_, op, _) => match op.as_str() {
                        "-n" => c.style(t, 2237, "Use [ -z .. ] instead of ! [ -n .. ]."),
                        "-z" => c.style(t, 2237, "Use [ -n .. ] instead of ! [ -z .. ]."),
                        _ => {}
                    },
                    Token::TC_Binary(style, op, _, _) => maybe_suggest_rewrite(c, false, *style, t, op),
                    _ => {}
                },
                // ! test -z x
                Token::T_SimpleCommand(_, words) => {
                    if let &[_, op, _] = &words[..] {
                        if is_command(ast, inner, "test") {
                            match get_literal_string(ast, op).as_deref() {
                                Some("-n") => c.style(t, 2237, "Use test -z .. instead of ! test -n .."),
                                Some("-z") => c.style(t, 2237, "Use test -n .. instead of ! test -z .."),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // test ! -n x
        Token::T_SimpleCommand(_, words) => {
            if let &[_, bang, op, _] = &words[..] {
                if is_command(ast, t, "test") && get_literal_string(ast, bang).as_deref() == Some("!") {
                    match get_literal_string(ast, op).as_deref() {
                        Some("-n") => c.style(t, 2236, "Use -z instead of ! -n."),
                        Some("-z") => c.style(t, 2236, "Use -n instead of ! -z."),
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}
