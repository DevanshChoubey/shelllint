// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for shelllint's own optional check `require-strict-mode` (SC9001, issue #409);
// no Haskell counterpart.

use shelllint_analyzer::analyzerlib::*;
use shelllint_analyzer::api::entry_test_checker;
use shelllint_analyzer::testing::find_entries;
use shelllint_checks::CHECKS;

/// (code, line, message) of every comment the check writes for `script`.
fn run(script: &str) -> Vec<(i64, u32, String)> {
    let entries = find_entries(CHECKS, "Analytics.hs", "checkRequireStrictMode");
    assert_eq!(entries.len(), 1);
    let checker = entry_test_checker(&entries);
    let mut pr = p_script(script);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    let spec = default_spec(&pr);
    let params = make_parameters(&spec, &pr.ast, root);
    run_checker(&params, &checker)
        .into_iter()
        .map(|tc| (tc.comment.code, pr.ast.span(tc.id).0.line, tc.comment.message))
        .collect()
}

fn missing(script: &str) -> Option<String> {
    let r = run(script);
    assert!(r.len() <= 1, "{r:?}");
    r.into_iter().next().map(|(code, line, m)| {
        assert_eq!((code, line), (9001, 1));
        m.split("missing ").nth(1).unwrap().split('.').next().unwrap().to_string()
    })
}

#[test]
fn nothing_set_bash() {
    assert_eq!(missing("#!/bin/bash\necho hi\n").as_deref(), Some("errexit, nounset, pipefail"));
}

#[test]
fn set_euo_pipefail() {
    assert_eq!(missing("#!/bin/bash\nset -euo pipefail\necho hi\n"), None);
}

#[test]
fn separate_short_flags_and_long_options() {
    assert_eq!(missing("#!/bin/bash\nset -e\nset -u\nset -o pipefail\n"), None);
    assert_eq!(missing("#!/bin/bash\nset -o errexit -o nounset -o pipefail\n"), None);
    assert_eq!(missing("#!/bin/bash\nset -eu -o pipefail\n"), None);
    assert_eq!(missing("#!/bin/bash\nset -o errexit\n").as_deref(), Some("nounset, pipefail"));
}

#[test]
fn set_e_does_not_imply_pipefail() {
    assert_eq!(missing("#!/bin/bash\nset -e\n").as_deref(), Some("nounset, pipefail"));
    assert_eq!(missing("#!/bin/bash\nset -o nounset\n").as_deref(), Some("errexit, pipefail"));
}

#[test]
fn set_inside_function_counts() {
    assert_eq!(missing("#!/bin/bash\nmain() { set -euo pipefail; }\nmain\n"), None);
}

#[test]
fn shebang_flags() {
    assert_eq!(missing("#!/bin/bash -e\n").as_deref(), Some("nounset, pipefail"));
    assert_eq!(missing("#!/bin/bash -eu\n").as_deref(), Some("pipefail"));
    assert_eq!(missing("#!/bin/bash -euo pipefail\n"), None);
    assert_eq!(missing("#!/bin/bash -e -u -o pipefail\n"), None);
    assert_eq!(missing("#!/usr/bin/env -S bash -euo pipefail\n"), None);
}

#[test]
fn ksh_requires_pipefail() {
    assert_eq!(missing("#!/bin/ksh\nset -eu\n").as_deref(), Some("pipefail"));
    assert_eq!(missing("#!/bin/ksh\nset -euo pipefail\n"), None);
}

#[test]
fn sh_and_dash_do_not_require_pipefail() {
    assert_eq!(missing("#!/bin/sh\nset -eu\n"), None);
    assert_eq!(missing("#!/bin/dash\nset -eu\n"), None);
    assert_eq!(missing("#!/bin/sh -eu\n"), None);
    assert_eq!(missing("#!/bin/sh\nset -e\n").as_deref(), Some("nounset"));
    assert_eq!(missing("#!/bin/sh\necho hi\n").as_deref(), Some("errexit, nounset"));
    assert_eq!(missing("#!/bin/dash\necho hi\n").as_deref(), Some("errexit, nounset"));
}

#[test]
fn reported_on_line_one_only_once() {
    let r = run("#!/bin/bash\n\n\necho a\necho b\n");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1, 1);
}

#[test]
fn not_enabled_by_default() {
    let optional = shelllint_analyzer::api::registered(CHECKS, "optional-tree");
    assert!(optional.iter().any(|e| e.optional.as_ref().is_some_and(|o| o.name == "require-strict-mode")));
}
