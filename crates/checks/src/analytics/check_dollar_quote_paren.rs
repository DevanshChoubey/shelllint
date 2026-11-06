// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDollarQuoteParen`, lines 4277-4284 (e11f964)

use crate::support::{fix_with, replace_start};
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDollarQuoteParen",
    registry: "node",
    order: 82,
    optional: None,
    kind: CheckKind::Node(check_dollar_quote_paren),
};

fn check_dollar_quote_paren(p: &Params<'_>, t: Id, c: &mut Comments) {
    // The token's own id is `t`, as T_DollarDoubleQuoted holds only its children.
    let Token::T_DollarDoubleQuoted(l) = p.ast.get(t) else { return };
    let Some(&first) = l.first() else { return };
    let Token::T_Literal(s) = p.ast.get(first) else { return };
    let Some(ch) = s.chars().next() else { return };
    if ch == '(' || ch == '{' {
        c.warn_with_fix(
            t,
            2247,
            "Flip leading $ and \" if this should be a quoted substitution.",
            fix(t, p),
        );
    }
}

fn fix(id: Id, params: &Params<'_>) -> shellcheck_core::interface::Fix {
    fix_with(vec![replace_start(id, params, 2, "\"$")])
}
