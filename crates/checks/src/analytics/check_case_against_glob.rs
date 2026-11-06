// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCaseAgainstGlob`, lines 1632-1643 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{is_glob, is_quoteable_expansion};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCaseAgainstGlob",
    registry: "node",
    order: 17,
    optional: None,
    kind: CheckKind::Node(check_case_against_glob),
};

fn check_case_against_glob(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_CaseExpression(_, cases) = p.ast.get(t) else { return };
    for (_, list, _) in cases {
        for &expr in list {
            check_(p, expr, c);
        }
    }
}

fn check_(p: &Params<'_>, expr: Id, c: &mut Comments) {
    if let Token::T_NormalWord(list) = p.ast.get(expr) {
        // If it's already a glob, assume that's what the user wanted
        if !is_glob(p.ast, expr) && list.iter().any(|&x| is_quoteable_expansion(p.ast, x)) {
            c.warn(
                expr,
                2254,
                "Quote expansions in case patterns to match literally rather than as a glob.",
            );
        }
    }
}
