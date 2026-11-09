// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--verbose` (extension beyond ShellCheck).

use std::process::{Command, Stdio};

fn run(dir: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .env("HOME", dir.join("nohome"))
        .env("XDG_CONFIG_HOME", dir.join("noxdg"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn setup(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-verbose-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("a")).unwrap();
    std::fs::write(d.join("a/x.sh"), "#!/bin/sh\necho $1\n").unwrap();
    std::fs::write(d.join("a/y.sh"), "#!/bin/sh\necho $1\n").unwrap();
    d
}

#[test]
fn stdout_and_exit_code_unchanged() {
    let d = setup("same");
    std::fs::write(d.join("a/.shellcheckrc"), "disable=SC2086\n").unwrap();
    for args in [&["a/x.sh", "a/y.sh"][..], &["--norc", "a/x.sh"], &["-f", "json", "a/x.sh"]] {
        let plain = run(&d, args);
        let mut v = vec!["--verbose"];
        v.extend_from_slice(args);
        let verbose = run(&d, &v);
        assert_eq!(plain.0, verbose.0, "{:?}", args);
        assert_eq!(plain.1, verbose.1, "{:?}", args);
        assert_eq!(plain.2, "", "{:?}", args);
    }
}

#[test]
fn stderr_names_file_and_rc() {
    let d = setup("rc");
    std::fs::write(d.join("a/.shellcheckrc"), "disable=SC2086\n").unwrap();
    let (code, out, err) = run(&d, &["--verbose", "a/x.sh", "a/y.sh"]);
    assert_eq!((code, out.as_str()), (0, ""));
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines.len(), 4, "{}", err);
    assert_eq!(lines[0], "shelllint: checking a/x.sh");
    assert!(lines[1].starts_with("shelllint: using rcfile ") && lines[1].ends_with("a/.shellcheckrc"), "{}", err);
    assert_eq!(lines[2], "shelllint: checking a/y.sh");
    assert_eq!(lines[1], lines[3]);
}

#[test]
fn stderr_says_none_without_rc() {
    let d = setup("none");
    let (code, _, err) = run(&d, &["--verbose", "a/x.sh"]);
    assert_eq!(code, 1);
    assert_eq!(err, "shelllint: checking a/x.sh\nshelllint: using rcfile none\n");
    std::fs::write(d.join("a/.shellcheckrc"), "disable=SC2086\n").unwrap();
    let (_, _, err) = run(&d, &["--verbose", "--norc", "a/x.sh"]);
    assert_eq!(err, "shelllint: checking a/x.sh\nshelllint: using rcfile none\n");
}

#[test]
fn stderr_names_rcfile_option() {
    let d = setup("rcopt");
    std::fs::write(d.join("my.rc"), "disable=SC2086\n").unwrap();
    let (code, _, err) = run(&d, &["--verbose", "--rcfile=my.rc", "a/x.sh"]);
    assert_eq!(code, 0);
    assert_eq!(err, "shelllint: checking a/x.sh\nshelllint: using rcfile my.rc\n");
}
