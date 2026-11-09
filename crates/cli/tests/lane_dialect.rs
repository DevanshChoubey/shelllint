// SPDX-License-Identifier: GPL-3.0-or-later
// CLI tests for `--modern-dialects`.

use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: &str) -> (i32, String) {
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
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn flag_off_warns() {
    let (code, out) = run(&["-i", "SC3045", "-f", "gcc", "-"], "#!/bin/sh\nread -d '' x\n");
    assert_eq!(code, 1);
    assert!(out.contains("SC3045"), "{out}");
}

#[test]
fn flag_on_is_clean() {
    let (code, out) = run(&["--modern-dialects", "-i", "SC3045", "-f", "gcc", "-"], "#!/bin/sh\nread -d '' x\n");
    assert_eq!((code, out.as_str()), (0, ""));
}

#[test]
fn flag_on_assoc_pairs_clean_off_warns() {
    let src = "#!/bin/bash\ndeclare -A a\na=(k v)\n";
    assert_eq!(run(&["--modern-dialects", "-i", "SC2190", "-"], src), (0, String::new()));
    assert_eq!(run(&["-i", "SC2190", "-f", "gcc", "-"], src).0, 1);
}
