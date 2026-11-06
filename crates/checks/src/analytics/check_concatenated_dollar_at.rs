// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkConcatenatedDollarAt`, lines 988-997 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_quote_free};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_word_parts, is_array_expansion};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkConcatenatedDollarAt",
    registry: "node",
    order: 63,
    optional: None,
    kind: CheckKind::Node(check_concatenated_dollar_at),
};

fn check_concatenated_dollar_at(p: &Params<'_>, word: Id, c: &mut Comments) {
    if !matches!(p.ast.get(word), Token::T_NormalWord(..)) {
        return;
    }
    let parts = get_word_parts(p.ast, word);
    // Haskell: not $ (isQuoteFree ... || null (drop 1 parts))
    if !(is_quote_free(p.ast, p.shell_type, word) || parts.len() <= 1) {
        let array = parts.iter().copied().find(|&x| is_array_expansion(p.ast, x));
        if let Some(t) = array {
            c.err(t, 2145, "Argument mixes string and array. Use * or separate argument.");
        }
    }
}
