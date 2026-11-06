// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkComparisonAgainstGlob`, lines 1612-1628 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::is_glob;
use shellcheck_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkComparisonAgainstGlob",
    registry: "node",
    order: 16,
    optional: None,
    kind: CheckKind::Node(check_comparison_against_glob),
};

fn check_comparison_against_glob(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Binary(typ, op, _, word) = p.ast.get(t) else { return };
    let word = *word;
    let op_ok = matches!(op.as_str(), "=" | "==" | "!=");
    if *typ == ConditionType::DoubleBracket && op_ok {
        if let Token::T_NormalWord(parts) = p.ast.get(word) {
            if let [only] = &parts[..] {
                if matches!(p.ast.get(*only), Token::T_DollarBraced(..)) {
                    c.warn(
                        word,
                        2053,
                        format!("Quote the right-hand side of {} in [[ ]] to prevent glob matching.", op),
                    );
                    return;
                }
            }
        }
    }
    if *typ == ConditionType::SingleBracket && op_ok && is_glob(p.ast, word) {
        let msg = if matches!(p.shell_type, Shell::Bash | Shell::Ksh) {
            // Busybox does not support glob matching
            "[ .. ] can't match globs. Use [[ .. ]] or case statement."
        } else {
            "[ .. ] can't match globs. Use a case statement."
        };
        c.err(word, 2081, msg);
        return;
    }
    if *typ == ConditionType::DoubleBracket
        && p.shell_type == Shell::BusyboxSh
        && op_ok
        && is_glob(p.ast, word)
    {
        c.err(word, 2330, "BusyBox [[ .. ]] does not support glob matching. Use a case statement.");
    }
}
