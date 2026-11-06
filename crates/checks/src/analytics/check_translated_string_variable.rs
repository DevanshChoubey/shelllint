// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTranslatedStringVariable`, lines 4290-4298 (e11f964)

use crate::support::{fix_with, replace_start};
use shellcheck_analyzer::analyzerlib::{Comments, Params, StackData};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{is_variable_char, is_variable_name};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTranslatedStringVariable",
    registry: "node",
    order: 84,
    optional: None,
    kind: CheckKind::Node(check_translated_string_variable),
};

fn check_translated_string_variable(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_DollarDoubleQuoted(l) = p.ast.get(t) else { return };
    let &[lit] = &l[..] else { return };
    let Token::T_Literal(s) = p.ast.get(lit) else { return };
    if !s.chars().all(is_variable_char) {
        return;
    }
    let member = p.variable_flow.iter().any(|d| match d {
        StackData::Assignment(_, _, name, _) => is_variable_name(name) && name == s,
        _ => false,
    });
    if member {
        c.warn_with_fix(
            t,
            2256,
            "This translated string is the name of a variable. Flip leading $ and \" if this should be a quoted substitution.",
            fix_with(vec![replace_start(t, p, 2, "\"$")]),
        );
    }
}
