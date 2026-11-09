// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the optional check `arith-errexit` (SC9003), shelllint's own.

use shelllint_checker::check_script;
use shelllint_core::interface::{CheckSpec, Code, Shell, empty_check_spec, mocked_system_interface};

fn run(script: &str, enable: bool) -> Vec<Code> {
    let spec = CheckSpec {
        filename: "file.sh".into(),
        script: script.into(),
        ignore_rc: true,
        shell_type_override: Some(Shell::Bash),
        optional_checks: if enable { vec!["arith-errexit".into()] } else { vec![] },
        ..empty_check_spec()
    };
    let sys = mocked_system_interface(vec![]);
    let mut v: Vec<Code> = check_script(&sys, &spec).comments.into_iter().map(|pc| pc.comment.code).collect();
    v.sort();
    v
}

fn flagged(script: &str) -> bool {
    run(script, true).contains(&9003)
}

#[test]
fn post_increment_under_set_e_is_flagged() {
    assert!(flagged("set -e\na=0\n((a++))\n"));
    assert!(flagged("set -o errexit\n((a--))\n"));
    assert!(flagged("set -e\nf() { ((n++)); }\n"));
    assert!(flagged("set -e\ntrue && ((a++))\n"));
}

#[test]
fn off_by_default_and_without_errexit() {
    assert!(!run("set -e\na=0\n((a++))\n", false).contains(&9003));
    assert!(!flagged("a=0\n((a++))\n"));
}

#[test]
fn conditions_and_guards_are_not_flagged() {
    assert!(!flagged("set -e\nif ((a++)); then :; fi\n"));
    assert!(!flagged("set -e\nwhile ((a++ < 3)); do :; done\n"));
    assert!(!flagged("set -e\nuntil ((a++ > 3)); do :; done\n"));
    assert!(!flagged("set -e\n((a++)) && echo x\n"));
    assert!(!flagged("set -e\n((a++)) || true\n"));
    assert!(!flagged("set -e\n{ ((a++)); } || true\n"));
    assert!(!flagged("set -e\n! ((a++))\n"));
}

#[test]
fn provably_nonzero_is_not_flagged() {
    assert!(!flagged("set -e\n((a = 5))\n"));
    assert!(!flagged("set -e\n((1))\n"));
    assert!(!flagged("set -e\n((a=0, 2))\n"));
    assert!(flagged("set -e\n((0))\n"));
    assert!(flagged("set -e\n((a = 0))\n"));
}
