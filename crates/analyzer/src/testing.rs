// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the check test helpers of ShellCheck (e11f964): Analytics.hs
// `verify`/`verifyNot`/`verifyTree`/`verifyNotTree`/`verifyCodes`/
// `verifyCodesTree`/`producesComments`/`runAndGetComments`,
// Checks/Commands.hs `verify`/`verifyNot`, Checks/ShellSupport.hs
// `verify`/`verifyNot`, all built on AnalyzerLib.hs `runAndGetCodes`.
//
// One entry point for the conformance harness: `run_check` runs a single
// registered check on a script and returns the sorted codes it produced
// (after `filterByAnnotation`). The harness turns that into the helper's
// verdict: `verify` = non-empty, `verifyNot` = empty, `verifyCodes` = equal.

use crate::analyzerlib::run_and_get_codes;
use crate::api::{CHECKS, CheckEntry, CheckKind, CommandName, entry_test_checker};
use shellcheck_core::interface::Code;

/// Resolve a test's check expression to registry entries.
///
/// `name` is the Haskell expression the test applies its helper to, without
/// the outer parentheses: `checkEchoWc`, or for parametrised command checks
/// `checkSudoArgs "sudo"`. Entries whose `name` equals the whole string win;
/// otherwise `f "arg"` selects the `Command` entries named `f` whose
/// `CommandName` is `arg`.
pub fn find_entries(module: &str, name: &str) -> Vec<&'static CheckEntry> {
    let exact: Vec<_> = CHECKS.iter().filter(|e| e.module == module && e.name == name).collect();
    if !exact.is_empty() {
        return exact;
    }
    let Some((f, arg)) = name.split_once(' ') else { return vec![] };
    let Some(arg) = arg.trim().strip_prefix('"').and_then(|a| a.strip_suffix('"')) else {
        return vec![];
    };
    CHECKS
        .iter()
        .filter(|e| {
            e.module == module
                && e.name == f
                && matches!(e.kind,
                    CheckKind::Command(CommandName::Exactly(n) | CommandName::Basename(n), _) if n == arg)
        })
        .collect()
}

/// Run one check (`module`, `name`, see `find_entries`) on `script`, the way
/// its module's test helper does, and return the sorted codes.
///
/// `kind` is the corpus kind: "node" (`verify`/`verifyNot`), "tree"
/// (`verifyTree`/`verifyNotTree`) or "codes" (`verifyCodes`).
///
/// Returns `None` when no entry matches (not ported yet). Panics if the
/// script does not parse (every Haskell helper then fails, so a passing
/// Haskell test never gets here) or if `kind` contradicts the entry's
/// `CheckKind`.
pub fn run_check(module: &str, name: &str, kind: &str, script: &str) -> Option<Vec<Code>> {
    let entries = find_entries(module, name);
    if entries.is_empty() {
        return None;
    }
    for e in &entries {
        let is_tree = matches!(e.kind, CheckKind::Tree(_));
        let ok = match kind {
            "node" => !is_tree,
            "tree" => is_tree,
            "codes" => true,
            _ => panic!("run_check: unknown kind {kind:?}"),
        };
        assert!(ok, "run_check: {module}:{name} is registered with the wrong CheckKind for a {kind:?} test");
    }
    let checker = entry_test_checker(&entries);
    Some(
        run_and_get_codes(&checker, script)
            .unwrap_or_else(|| panic!("run_check: {module}:{name}: script does not parse")),
    )
}
