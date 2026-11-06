// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkLiteralBreakingTest`, lines 1498-1520 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_literal_string, get_word_parts, is_constant};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkLiteralBreakingTest",
    registry: "node",
    order: 11,
    optional: None,
    kind: CheckKind::Node(check_literal_breaking_test),
};

fn check_literal_breaking_test(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    let has_equals = |t: Id| match_token(p, |s| s.contains('='), t);
    let is_non_empty = |t: Id| match_token(p, |s| !s.is_empty(), t);

    let comparison_warning = |list: &[Id]| -> Option<(Id, i64, String)> {
        let token = list.iter().copied().find(|&x| has_equals(x))?;
        Some((token, 2077, "You need spaces around the comparison operator.".to_string()))
    };
    let tautology_warning = |t: Id, s: &str| -> Option<(Id, i64, String)> {
        let token = get_word_parts(ast, t).into_iter().find(|&x| is_non_empty(x))?;
        Some((token, 2157, s.to_string()))
    };

    let result = match ast.get(t) {
        Token::TC_Nullary(_, w) => match ast.get(*w) {
            Token::T_NormalWord(l) => {
                if is_constant(ast, *w) {
                    None // Covered by SC2078
                } else {
                    comparison_warning(l).or_else(|| {
                        tautology_warning(
                            *w,
                            "Argument to implicit -n is always true due to literal strings.",
                        )
                    })
                }
            }
            _ => None,
        },
        Token::TC_Unary(_, op, w) => match ast.get(*w) {
            Token::T_NormalWord(_) => match op.as_str() {
                "-n" => tautology_warning(*w, "Argument to -n is always true due to literal strings."),
                "-z" => tautology_warning(*w, "Argument to -z is always false due to literal strings."),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    if let Some((id, code, msg)) = result {
        c.err(id, code, msg);
    }
}

fn match_token(p: &Params<'_>, m: impl Fn(&str) -> bool, t: Id) -> bool {
    get_literal_string(p.ast, t).is_some_and(|s| m(&s))
}
