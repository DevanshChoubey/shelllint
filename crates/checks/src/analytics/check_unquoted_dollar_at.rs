// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnquotedDollarAt`, lines 976-982 (e11f964)

use shellcheck_analyzer::analyzerlib::{
    Comments, Params, is_quoted_alternative_reference, is_strictly_quote_free,
};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::is_array_expansion;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnquotedDollarAt",
    registry: "node",
    order: 5,
    optional: None,
    kind: CheckKind::Node(check_unquoted_dollar_at),
};

fn check_unquoted_dollar_at(p: &Params<'_>, word: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(word) else { return };
    if !is_strictly_quote_free(p.ast, p.shell_type, word) {
        if let Some(&x) = parts.iter().find(|&&x| is_array_expansion(p.ast, x)) {
            if !is_quoted_alternative_reference(p.ast, x) {
                c.err(x, 2068, "Double quote array expansions to avoid re-splitting elements.");
            }
        }
    }
}
