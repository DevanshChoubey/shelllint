// SPDX-License-Identifier: GPL-3.0-or-later
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart
//
// Regression guard: the checks crate registers into the analyzer's CHECKS
// slice via linkme, and its entries are silently dropped unless the binary
// links the crate (`use shellcheck_checks as _;` in src/lib.rs). If that
// breaks, every check reads as `unported` instead of failing.
#![cfg(feature = "analyzer")]

use shellcheck_conformance::runner::{Filter, run_all};
use shellcheck_conformance::{default_corpus_dir, load_corpus};

#[test]
fn checks_crate_is_linked() {
    assert!(
        shellcheck_analyzer::api::CHECKS
            .iter()
            .any(|e| e.module == "Analytics.hs" && e.name == "checkEchoWc"),
        "Analytics.hs:checkEchoWc missing from CHECKS: shellcheck-checks is not linked"
    );
}

#[test]
fn pilot_report_passes() {
    let recs = load_corpus(&default_corpus_dir().join("tests.jsonl")).unwrap();
    let f = Filter { target: Some("checkEchoWc".into()), ..Default::default() };
    let (c, _) = run_all(&recs, &f);
    assert_eq!((c.pass, c.fail, c.blocked, c.unported), (1, 0, 0, 0));
}
