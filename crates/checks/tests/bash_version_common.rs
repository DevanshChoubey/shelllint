// SPDX-License-Identifier: GPL-3.0-or-later
// Shared helpers of the `--bash-version` tests (SC9100-SC9137); no Haskell counterpart.
// Each test file says `mod bash_version_common;` and uses what it needs.
//
// Building a script and setting a target:
//
//     bash("3.2", "declare -A m")                // codes in 9100..=9199, target 3.2, shell bash
//     codes(Shell::Sh, Some("3.2"), script)      // any shell, optional target
//     messages("3.2", script)                    // the message texts instead
//     assert_pair(9102, "4.0", "declare -A m")   // fires (only that code) one release below 4.0, silent at 4.0
//     assert_silent("3.2", "declare -a m")       // a false-positive guard
//     previous("4.0")                            // "3.2": the release before a version

#![allow(dead_code)]

use shelllint_analyzer::analyzerlib::*;
use shelllint_analyzer::api::shell_support_checker;
use shelllint_checks::CHECKS;
use shelllint_core::interface::{AnalysisSpec, BashVersion, Code, Shell};

pub fn version(s: &str) -> BashVersion {
    s.parse().unwrap_or_else(|_| panic!("{s} is not a bash release"))
}

/// The release before `v` ("4.0" gives "3.2").
pub fn previous(v: &str) -> String {
    let v = version(v);
    let i = BashVersion::RELEASES.iter().position(|&(a, b)| (a, b) == (v.major(), v.minor())).unwrap();
    assert!(i > 0, "{v} is the oldest target");
    let (a, b) = BashVersion::RELEASES[i - 1];
    format!("{a}.{b}")
}

/// Every SC91xx comment, as (code, message), for a script run as `shell` with an optional target.
pub fn run(shell: Shell, target: Option<&str>, script: &str) -> Vec<(Code, String)> {
    let mut pr = p_script(script);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    let spec = AnalysisSpec { shell_type: Some(shell), bash_version: target.map(version), ..default_spec(&pr) };
    let params = make_parameters(&spec, &pr.ast, root);
    let checker = shell_support_checker(CHECKS, &params);
    run_checker(&params, &checker)
        .into_iter()
        .filter(|tc| (9100..=9199).contains(&tc.comment.code))
        .map(|tc| (tc.comment.code, tc.comment.message))
        .collect()
}

pub fn codes(shell: Shell, target: Option<&str>, script: &str) -> Vec<Code> {
    run(shell, target, script).into_iter().map(|(c, _)| c).collect()
}

/// SC91xx codes of a bash script with target `target`.
pub fn bash(target: &str, script: &str) -> Vec<Code> {
    codes(Shell::Bash, Some(target), script)
}

pub fn messages(target: &str, script: &str) -> Vec<String> {
    run(Shell::Bash, Some(target), script).into_iter().map(|(_, m)| m).collect()
}

/// `script` uses one feature introduced in `since`: it gives exactly `[code]` at the release
/// before it and nothing at `since` itself or later.
pub fn assert_pair(code: Code, since: &str, script: &str) {
    let below = previous(since);
    assert_eq!(bash(&below, script), [code], "{script:?} at target {below}");
    assert_eq!(bash(since, script), Vec::<Code>::new(), "{script:?} at target {since}");
    assert_eq!(bash("5.3", script), Vec::<Code>::new(), "{script:?} at target 5.3");
}

/// No SC91xx at all, even for the oldest target.
pub fn assert_silent(target: &str, script: &str) {
    assert_eq!(bash(target, script), Vec::<Code>::new(), "{script:?} at target {target}");
}
