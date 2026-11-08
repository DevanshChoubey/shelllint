// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnnecessaryArithmeticExpansionIndex`, lines 5322-5335 (e11f964)

use crate::support::{fix_with, replace_end, replace_start};
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnnecessaryArithmeticExpansionIndex",
    registry: "node",
    order: 98,
    optional: None,
    kind: CheckKind::Node(check_unnecessary_arithmetic_expansion_index),
};

fn check_unnecessary_arithmetic_expansion_index(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(_, _, var, _) = params.ast.get(t) else { return };
    let &[seq] = &var[..] else { return };
    let Token::TA_Sequence(s) = params.ast.get(seq) else { return };
    let &[exp] = &s[..] else { return };
    let Token::TA_Expansion(e) = params.ast.get(exp) else { return };
    let &[id] = &e[..] else { return };
    if !matches!(params.ast.get(id), Token::T_DollarArithmetic(_)) {
        return;
    }
    c.style_with_fix(
        id,
        2321,
        "Array indices are already arithmetic contexts. Prefer removing the $(( and )).",
        fix(id, params),
    );
}

fn fix(id: Id, params: &Params<'_>) -> shelllint_core::interface::Fix {
    fix_with(vec![
        replace_start(id, params, 3, ""), // Remove "$(("
        replace_end(id, params, 2, ""),   // Remove "))"
    ])
}
