// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the optional check `prefer-local` (SC9002); no Haskell counterpart.

use shelllint_analyzer::analyzerlib::*;
use shelllint_analyzer::api::{analytics_checker, analytics_optional_checks};
use shelllint_checks::CHECKS;
use shelllint_core::interface::{AnalysisSpec, Shell};

fn lines(shell: Shell, enabled: bool, script: &str) -> Vec<i64> {
    let mut pr = p_script(script);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    let optional_checks = if enabled { vec!["prefer-local".to_string()] } else { vec![] };
    let spec = AnalysisSpec { shell_type: Some(shell), optional_checks, ..default_spec(&pr) };
    let params = make_parameters(&spec, &pr.ast, root);
    let checker = analytics_checker(CHECKS, &spec);
    run_checker(&params, &checker)
        .into_iter()
        .filter(|tc| tc.comment.code == 9002)
        .map(|tc| pr.ast.span(tc.id).0.line as i64)
        .collect()
}

fn bash(script: &str) -> Vec<i64> {
    lines(Shell::Bash, true, script)
}

#[test]
fn registered_as_optional() {
    let d = analytics_optional_checks(CHECKS);
    assert!(d.iter().any(|d| d.name == "prefer-local" && d.id == [9002]));
}

#[test]
fn off_by_default() {
    assert!(lines(Shell::Bash, false, "f() { x=1; echo \"$x\"; }\n").is_empty());
}

#[test]
fn positive() {
    assert_eq!(bash("f() {\n  x=1\n  echo \"$x\"\n}\n"), [2]);
    assert_eq!(bash("f() {\n  x=1\n  x=2\n}\n"), [2]);
}

#[test]
fn read_outside_function() {
    assert!(bash("f() { x=1; }\nf\necho \"$x\"\n").is_empty());
    assert!(bash("f() { x=1; }\ng() { echo \"$x\"; }\n").is_empty());
}

#[test]
fn local_forms() {
    assert!(bash("f() { local x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { local x; x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { declare x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { typeset x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { local -r x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { declare -g x=1; echo \"$x\"; }").is_empty());
    assert!(bash("f() { export x=1; }").is_empty());
    assert!(bash("f() { x=1; export x; }").is_empty());
    assert!(bash("f() { readonly x=1; }").is_empty());
    assert!(bash("x=1\n").is_empty());
}

#[test]
fn loop_variables() {
    assert_eq!(bash("f() {\n  for i in 1 2; do echo \"$i\"; done\n}\n"), [2]);
    assert!(bash("f() { local i; for i in 1 2; do echo \"$i\"; done; }").is_empty());
    assert!(bash("f() { for i in 1 2; do :; done; }\nf\necho \"$i\"").is_empty());
}

#[test]
fn not_in_plain_sh() {
    assert!(lines(Shell::Sh, true, "f() { x=1; echo \"$x\"; }\n").is_empty());
    assert_eq!(lines(Shell::Dash, true, "f() { x=1; echo \"$x\"; }\n"), [1]);
}
