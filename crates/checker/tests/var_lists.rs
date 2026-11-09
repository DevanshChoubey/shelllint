// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `safe-vars` and `strict-env-vars` (an extension beyond upstream).

use shelllint_checker::check_script;
use shelllint_core::interface::{CheckSpec, Code, empty_check_spec, mock_rc_file, mocked_system_interface};

fn codes(rc: Option<&str>, spec: CheckSpec) -> Vec<Code> {
    let sys = match rc {
        Some(rc) => mock_rc_file(rc, mocked_system_interface(vec![])),
        None => mocked_system_interface(vec![]),
    };
    check_script(&sys, &spec).comments.into_iter().map(|c| c.comment.code).collect()
}

fn spec(script: &str) -> CheckSpec {
    CheckSpec { script: script.into(), ..empty_check_spec() }
}

const UNQUOTED: &str = "#!/bin/sh\necho $FOO $BAR\n";
const ARRAY: &str = "#!/bin/bash\narr=($FOO)\necho \"${arr[@]}\"\n";
const UPPER: &str = "#!/bin/sh\necho \"$FOO\" \"$HOME\" \"$lower\"\n";

#[test]
fn safe_vars_off_warns() {
    assert_eq!(codes(None, spec(UNQUOTED)), vec![2086, 2086]);
    assert_eq!(codes(None, spec(ARRAY)), vec![2206]);
}

#[test]
fn safe_vars_flag() {
    let s = CheckSpec { safe_vars: vec!["FOO".into()], ..spec(UNQUOTED) };
    assert_eq!(codes(None, s), vec![2086]);
    let s = CheckSpec { safe_vars: vec!["FOO".into(), "BAR".into()], ..spec(UNQUOTED) };
    assert_eq!(codes(None, s), Vec::<Code>::new());
    let s = CheckSpec { safe_vars: vec!["FOO".into()], ..spec(ARRAY) };
    assert_eq!(codes(None, s), Vec::<Code>::new());
}

#[test]
fn safe_vars_rc() {
    assert_eq!(codes(Some("safe-vars=FOO,BAR"), spec(UNQUOTED)), Vec::<Code>::new());
    assert_eq!(codes(Some("safe-vars=FOO"), spec(UNQUOTED)), vec![2086]);
    assert_eq!(codes(Some("safe-vars=FOO"), spec(ARRAY)), Vec::<Code>::new());
}

#[test]
fn safe_vars_without_extended_analysis() {
    let off = CheckSpec { extended_analysis: Some(false), ..spec(UNQUOTED) };
    assert_eq!(codes(None, off), vec![2086, 2086]);
    let on = CheckSpec { extended_analysis: Some(false), safe_vars: vec!["FOO".into()], ..spec(UNQUOTED) };
    assert_eq!(codes(None, on), vec![2086]);
}

#[test]
fn safe_vars_do_not_hide_assignments() {
    let s = CheckSpec { safe_vars: vec!["FOO".into()], ..spec("#!/bin/sh\nFOO=$(date)\necho $FOO\n") };
    assert_eq!(codes(None, s), vec![2086]);
}

#[test]
fn strict_env_vars_off() {
    assert_eq!(codes(None, spec(UPPER)), vec![2154]);
}

#[test]
fn strict_env_vars_flag() {
    let s = CheckSpec { strict_env_vars: Some(true), ..spec(UPPER) };
    assert_eq!(codes(None, s), vec![2154, 2154]);
}

#[test]
fn strict_env_vars_rc() {
    assert_eq!(codes(Some("strict-env-vars=true"), spec(UPPER)), vec![2154, 2154]);
    assert_eq!(codes(Some("strict-env-vars=false"), spec(UPPER)), vec![2154]);
    assert_eq!(codes(Some("strict-env-vars=false"), CheckSpec { strict_env_vars: None, ..spec(UPPER) }), vec![2154]);
}
