// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnnecessaryParens`, lines 5344-5366 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

use crate::support::{fix_with, replace_end, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnnecessaryParens",
    registry: "node",
    order: 99,
    optional: None,
    kind: CheckKind::Node(check_unnecessary_parens),
};

fn check_unnecessary_parens(params: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = params.ast;

    let fix = |id: Id| {
        fix_with(vec![
            replace_start(id, params, 1, ""), // Remove "("
            replace_end(id, params, 1, ""),   // Remove ")"
        ])
    };

    let check_leading = |c: &mut Comments, s: &str, t: Id| {
        if let Token::TA_Sequence(l) = ast.get(t) {
            if let &[id] = &l[..] {
                if let Token::TA_Parenthesis(_) = ast.get(id) {
                    c.style_with_fix(
                        id,
                        2323,
                        format!("{}. Prefer not wrapping in additional parentheses.", s),
                        fix(id),
                    );
                }
            }
        }
    };

    match ast.get(t) {
        Token::T_DollarArithmetic(t) => {
            check_leading(c, "$(( (x) )) is the same as $(( x ))", *t)
        }
        Token::T_ForArithmetic(x, y, z, _) => {
            for &t in &[*x, *y, *z] {
                check_leading(
                    c,
                    "for (((x); (y); (z))) is the same as for ((x; y; z))",
                    t,
                );
            }
        }
        Token::T_Assignment(_, _, idx, _) => {
            if let &[t] = &idx[..] {
                check_leading(c, "a[(x)] is the same as a[x]", t);
            }
        }
        Token::T_Arithmetic(t) => check_leading(c, "(( (x) )) is the same as (( x ))", *t),
        Token::TA_Parenthesis(inner) => {
            if let Token::TA_Sequence(l) = ast.get(*inner) {
                if let &[id] = &l[..] {
                    if let Token::TA_Parenthesis(_) = ast.get(id) {
                        c.style_with_fix(
                            id,
                            2322,
                            "In arithmetic contexts, ((x)) is the same as (x). Prefer only one layer of parentheses.",
                            fix(id),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}
