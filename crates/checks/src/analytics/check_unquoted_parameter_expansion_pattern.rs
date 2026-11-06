// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnquotedParameterExpansionPattern`, lines 4917-4937 (e11f964)

use crate::support::surround_with;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, oversimplify};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnquotedParameterExpansionPattern",
    registry: "node",
    order: 93,
    optional: None,
    kind: CheckKind::Node(check_unquoted_parameter_expansion_pattern),
};

fn check_unquoted_parameter_expansion_pattern(p: &Params<'_>, x: Id, c: &mut Comments) {
    let Token::T_DollarBraced(true, word) = p.ast.get(x) else { return };
    let word = *word;
    let Token::T_NormalWord(parts) = p.ast.get(word) else { return };
    let [first, rest @ ..] = &parts[..] else { return };
    if rest.is_empty() || !matches!(p.ast.get(*first), Token::T_Literal(_)) {
        return;
    }
    let modifier = get_braced_modifier(&oversimplify(p.ast, word).concat());
    if modifier.starts_with('%') || modifier.starts_with('#') {
        for &t in rest {
            check(p, t, c);
        }
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_DollarBraced(..) | Token::T_DollarExpansion(..) | Token::T_Backticked(..) => inform(p, t, c),
        _ => {}
    }
}

fn inform(p: &Params<'_>, t: Id, c: &mut Comments) {
    c.info_with_fix(
        t,
        2295,
        "Expansions inside ${..} need to be quoted separately, otherwise they match as patterns.",
        surround_with(t, p, "\""),
    );
}
