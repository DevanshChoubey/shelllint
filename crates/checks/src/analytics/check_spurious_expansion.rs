// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSpuriousExpansion`, lines 2078-2090 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSpuriousExpansion",
    registry: "node",
    order: 39,
    optional: None,
    kind: CheckKind::Node(check_spurious_expansion),
};

fn check_spurious_expansion(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let &[w] = &words[..] else { return };
    let Token::T_NormalWord(parts) = p.ast.get(w) else { return };
    let &[word] = &parts[..] else { return };
    match p.ast.get(word) {
        Token::T_DollarExpansion(..) => c.warn(
            word,
            2091,
            "Remove surrounding $() to avoid executing output (or use eval if intentional).",
        ),
        Token::T_Backticked(..) => c.warn(
            word,
            2092,
            "Remove backticks to avoid executing output (or use eval if intentional).",
        ),
        Token::T_DollarArithmetic(..) => c.err(
            word,
            2084,
            "Remove '$' or use '_=$((expr))' to avoid executing output.",
        ),
        _ => {}
    }
}
