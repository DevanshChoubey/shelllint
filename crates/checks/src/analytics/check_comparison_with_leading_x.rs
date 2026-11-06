// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkComparisonWithLeadingX`, lines 4553-4584 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_literal_string, get_word_parts};
use shellcheck_core::interface::Replacement;

use crate::support::{fix_with, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkComparisonWithLeadingX",
    registry: "node",
    order: 91,
    optional: None,
    kind: CheckKind::Node(check_comparison_with_leading_x),
};

fn check_comparison_with_leading_x(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::TC_Binary(_, op, lhs, rhs) if matches!(op.as_str(), "=" | "==" | "!=") => {
            check(p, c, *lhs, *rhs);
        }
        Token::T_SimpleCommand(_, words) => {
            if let &[cmd, lhs, op, rhs] = &words[..] {
                if get_literal_string(p.ast, cmd).as_deref() == Some("test")
                    && matches!(get_literal_string(p.ast, op).as_deref(), Some("=" | "==" | "!="))
                {
                    check(p, c, lhs, rhs);
                }
            }
        }
        _ => {}
    }
}

const MSG: &str = "Avoid x-prefix in comparisons as it no longer serves a purpose.";

fn check(p: &Params<'_>, c: &mut Comments, lhs: Id, rhs: Id) {
    let Some(l) = fix_leading_x(p, lhs) else { return };
    let Some(r) = fix_leading_x(p, rhs) else { return };
    c.style_with_fix(lhs, 2268, MSG, fix_with(vec![l, r]));
}

fn fix_leading_x(p: &Params<'_>, token: Id) -> Option<Replacement> {
    let parts = get_word_parts(p.ast, token);
    let &first = parts.first()?;
    match p.ast.get(first) {
        Token::T_Literal(s) if s.chars().next().is_some_and(|c| c.to_lowercase().eq(['x'])) => {
            // The side is a single, unquoted x or X, so we have to quote
            if let Token::T_NormalWord(l) = p.ast.get(token) {
                if let &[only] = &l[..] {
                    if let Token::T_Literal(s1) = p.ast.get(only) {
                        if s1.chars().count() == 1 {
                            return Some(replace_start(only, p, 1, "\"\""));
                        }
                    }
                }
            }
            // Otherwise we can just delete it
            Some(replace_start(first, p, 1, ""))
        }
        Token::T_SingleQuoted(s) if s.chars().next().is_some_and(|c| c.to_lowercase().eq(['x'])) => {
            // Replace the single quote and the character x or X
            Some(replace_start(first, p, 2, "'"))
        }
        _ => None,
    }
}
