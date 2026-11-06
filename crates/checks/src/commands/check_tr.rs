// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkTr`, lines 236-255 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string, is_glob};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkTr",
    registry: "command",
    order: 0,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("tr"), check_tr),
};

/// `mapM_ f . arguments`
fn check_tr(p: &Params<'_>, t: Id, c: &mut Comments) {
    for w in arguments(p.ast, t) {
        f(p, w, c);
    }
}

fn f(p: &Params<'_>, word: Id, c: &mut Comments) {
    if is_glob(p.ast, word) {
        // The user will go [ab] -> '[ab]' -> 'ab'. Fixme?
        c.warn(word, 2060, "Quote parameters to tr to prevent glob expansion.");
        return;
    }
    match get_literal_string(p.ast, word).as_deref() {
        Some("a-z") => c.info(word, 2018, "Use '[:lower:]' to support accents and foreign alphabets."),
        Some("A-Z") => c.info(word, 2019, "Use '[:upper:]' to support accents and foreign alphabets."),
        Some(s) => {
            // Eliminate false positives by only looking for dupes in SET2?
            if !(s.starts_with('-') || s.contains("[:")) && duplicated(s) {
                c.info(word, 2020, "tr replaces sets of chars, not words (mentioned due to duplicates).");
            }
            if !(s.starts_with("[:") || s.starts_with("[=")) && s.starts_with('[')
                && s.ends_with(']')
                && s.chars().count() > 2
                && !s.contains('*')
            {
                c.info(word, 2021, "Don't use [] around classes in tr, it replaces literal square brackets.");
            }
        }
        None => {}
    }
}

/// `relevant /= nub relevant` for the letters of `s`.
fn duplicated(s: &str) -> bool {
    let relevant: Vec<char> = s.chars().filter(|&c| shellcheck_core::unicode::is_alpha(c)).collect();
    relevant.iter().enumerate().any(|(i, c)| relevant[..i].contains(c))
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn codes(script: &str) -> Vec<u32> {
        run_check("Checks/Commands.hs", "checkTr", "node", script)
            .expect("registered")
            .into_iter()
            .map(|c| c as u32)
            .collect()
    }

    #[test]
    fn duplicated_letters_use_general_category() {
        // Repeated Devanagari vowel signs (Mc) are not `isAlpha`: no SC2020.
        assert!(!codes("tr xाा y").contains(&2020));
        // Repeated letters in other scripts are.
        assert!(codes("tr नाना ab").contains(&2020));
        assert!(codes("tr ǅaǅ x").contains(&2020));
    }
}
