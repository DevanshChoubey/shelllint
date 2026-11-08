// SPDX-License-Identifier: GPL-3.0-or-later
// Direct tests for engine-adjacent helpers of Parser.hs (e11f964):
// parseForgettingContext (inSeparateContext / forgetOnFailure) and the
// un-escaping done by readBackTicked.

use shelllint_core::ast::Token;
use shelllint_core::interface::Severity::ErrorC;
use shelllint_parser::testing::run_test_parser;

#[test]
fn forget_on_failure_drops_problems_on_failure() {
    let (r, _, sys) = run_test_parser(
        |p| {
            let pos = p.get_position();
            p.alt(
                |p| {
                    p.forget_on_failure(|p| {
                        p.parse_problem_at(pos, ErrorC, 9001, "inner");
                        p.fail::<()>("boom")
                    })
                },
                |_| Ok(()),
            )
        },
        "",
    );
    assert!(r.is_ok());
    assert!(sys.parse_problems.is_empty(), "{:?}", sys.parse_problems);
}

#[test]
fn forget_on_failure_keeps_problems_on_success() {
    let (r, _, sys) = run_test_parser(
        |p| {
            let pos = p.get_position();
            p.forget_on_failure(|p| {
                p.parse_problem_at(pos, ErrorC, 9001, "inner");
                Ok(())
            })
        },
        "",
    );
    assert!(r.is_ok());
    assert_eq!(sys.parse_problems.len(), 1);
    assert_eq!(sys.parse_problems[0].code, 9001);
}

#[test]
fn in_separate_context_restores_sys_on_success() {
    let (r, _, sys) = run_test_parser(
        |p| {
            let pos = p.get_position();
            p.in_separate_context(|p| {
                p.parse_problem_at(pos, ErrorC, 9001, "inner");
                Ok(())
            })
        },
        "",
    );
    assert!(r.is_ok());
    assert!(sys.parse_problems.is_empty(), "{:?}", sys.parse_problems);
}

/// Every token reachable from the root of a `readBackTicked False` parse
/// (ids abandoned by backtracking stay unset, as in Haskell).
fn tokens_of(input: &str) -> Vec<Token> {
    let (r, user, _) = run_test_parser(|p| p.read_back_ticked(false), input);
    let root = r.unwrap_or_else(|e| panic!("{input:?}: {e:?}"));
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(user.ast.get(id).clone());
        stack.extend(user.ast.children(id));
    }
    out
}

#[test]
fn back_ticked_nested_escaped_backticks() {
    let toks = tokens_of("`echo \\`foo\\``");
    let n = toks.iter().filter(|t| matches!(t, Token::T_Backticked(_))).count();
    assert_eq!(n, 2, "outer and inner backtick expansions: {toks:?}");
    assert!(toks.contains(&Token::T_Literal("foo".to_string())), "{toks:?}");
}

#[test]
fn back_ticked_unescapes_dollar() {
    let toks = tokens_of("`echo \\$x`");
    assert!(toks.iter().any(|t| matches!(t, Token::T_DollarBraced(..))), "{toks:?}");
    assert!(!toks.iter().any(|t| matches!(t, Token::T_Literal(s) if s.contains('\\'))), "{toks:?}");
}
