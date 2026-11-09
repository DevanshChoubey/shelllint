// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the shelllint-only optional check `unused-function` (SC9005).

use shelllint_checker::check_script_owned;
use shelllint_core::interface::{CheckSpec, Code, Shell, empty_check_spec, mocked_system_interface};

fn run(script: &str, optional: &[&str]) -> Vec<Code> {
    let spec = CheckSpec {
        filename: "script.sh".to_string(),
        script: script.to_string(),
        ignore_rc: true,
        shell_type_override: Some(Shell::Bash),
        optional_checks: optional.iter().map(|s| s.to_string()).collect(),
        ..empty_check_spec()
    };
    check_script_owned(&mocked_system_interface(vec![]), spec).comments.into_iter().map(|pc| pc.comment.code).collect()
}

fn on(script: &str) -> Vec<Code> {
    run(script, &["unused-function"])
}

#[test]
fn used_function_is_quiet() {
    assert_eq!(on("foo() { echo hi; }\nfoo\n"), Vec::<Code>::new());
}

#[test]
fn unused_function_is_reported() {
    assert_eq!(on("foo() { echo hi; }\nbar() { foo; }\nfoo\n"), vec![9005]);
}

#[test]
fn trap_counts_as_use() {
    assert_eq!(on("cleanup() { rm -f x; }\ntrap cleanup EXIT\n"), Vec::<Code>::new());
    assert_eq!(on("cleanup() { rm -f x; }\ntrap 'cleanup; exit' INT\n"), Vec::<Code>::new());
}

#[test]
fn dynamic_command_name_disables_the_check() {
    assert_eq!(on("foo() { echo hi; }\ncmd=foo\n\"$cmd\"\n"), Vec::<Code>::new());
    assert_eq!(on("foo() { echo hi; }\neval \"$1\"\n"), Vec::<Code>::new());
}

#[test]
fn off_by_default() {
    assert_eq!(run("foo() { echo hi; }\n", &[]), Vec::<Code>::new());
    assert_eq!(run("foo() { echo hi; }\n", &["all"]), Vec::<Code>::new());
    assert_eq!(run("foo() { echo hi; }\n", &["all", "unused-function"]), vec![9005]);
}
