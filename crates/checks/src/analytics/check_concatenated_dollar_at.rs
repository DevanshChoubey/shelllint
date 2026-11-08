// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkConcatenatedDollarAt`, lines 988-997 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, is_quote_free};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_word_parts, is_array_expansion};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
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
