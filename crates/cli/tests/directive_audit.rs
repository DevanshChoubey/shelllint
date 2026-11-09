// SPDX-License-Identifier: GPL-3.0-or-later
// CLI tests for `--ignore-directives` and `--report-unused-disables`,
// extensions beyond upstream ShellCheck.

use std::io::Write;
use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
}

fn run(args: &[&str], stdin: &str) -> Out {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let o = child.wait_with_output().unwrap();
    Out { code: o.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&o.stdout).into_owned() }
}

const SCRIPT: &str = "#!/bin/sh\n# shellcheck disable=SC2034,SC2086\necho $1\n";

#[test]
fn without_the_flags_nothing_changes() {
    let o = run(&["-f", "gcc", "-"], SCRIPT);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
}

#[test]
fn ignore_directives_shows_suppressed_warnings() {
    let o = run(&["-f", "gcc", "--ignore-directives", "-"], SCRIPT);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, "-:3:6: note: Double quote to prevent globbing and word splitting. [SC2086]\n");
}

#[test]
fn report_unused_disables_names_the_dead_code() {
    let o = run(&["-f", "gcc", "--report-unused-disables", "-"], SCRIPT);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, "-:2:1: note: This disable directive suppresses nothing: SC2034 [SC9006]\n");
}

#[test]
fn report_can_be_excluded_like_any_code() {
    let o = run(&["-f", "gcc", "--report-unused-disables", "-e", "SC9006", "-"], SCRIPT);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
}

#[test]
fn severity_filter_keeps_directives_used() {
    // -S warning hides the info-level SC9006 and must not make anything else appear.
    let o = run(&["-f", "gcc", "--report-unused-disables", "-S", "warning", "-"], SCRIPT);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
}
