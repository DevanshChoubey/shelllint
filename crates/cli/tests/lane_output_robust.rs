// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--tab-width`, early-closed stdout and non-UTF-8 input.

use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shelllint"));
    cmd.args(args).env_remove("SHELLCHECK_OPTS").stdin(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().unwrap();
    Out {
        code: o.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn tmpfile(name: &str, contents: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("shelllint-lane-output-robust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, contents).unwrap();
    p.to_string_lossy().into_owned()
}

const TABBED: &[u8] = b"#!/bin/sh\n\techo $x\n";

/// The lines of the tty report that follow the "In ... line" header.
fn echo_and_caret(stdout: &str) -> (String, String) {
    let lines: Vec<&str> = stdout.lines().collect();
    let i = lines.iter().position(|l| l.starts_with("In ")).unwrap();
    (lines[i + 1].to_string(), lines[i + 2].to_string())
}

#[test]
fn tab_width_aligns_carets() {
    let f = tmpfile("tabbed.sh", TABBED);
    let o = run(&["--tab-width=4", "--color=never", &f], &[]);
    assert_eq!(o.code, 1);
    let (src, caret) = echo_and_caret(&o.stdout);
    assert_eq!(src, "    echo $x");
    // Column of `$x` is 10 under 4-wide stops, so the caret has 9 spaces.
    assert_eq!(caret.find('^'), src.find("$x"));
    assert!(!o.stdout.contains('\t') || o.stdout.contains("Did you mean"));
}

#[test]
fn tab_width_separate_argument() {
    let f = tmpfile("tabbed2.sh", TABBED);
    let o = run(&["--tab-width", "2", "--color=never", &f], &[]);
    let (src, caret) = echo_and_caret(&o.stdout);
    assert_eq!(src, "  echo $x");
    assert_eq!(caret.find('^'), src.find("$x"));
}

#[test]
fn without_tab_width_output_keeps_tabs() {
    let f = tmpfile("tabbed3.sh", TABBED);
    let o = run(&["--color=never", &f], &[]);
    let (src, caret) = echo_and_caret(&o.stdout);
    assert_eq!(src, "\techo $x");
    assert_eq!(caret, format!("{}^-- SC2154 (warning): x is referenced but not assigned.", " ".repeat(13)));
    // Passing the 8-wide default equals the parser's own tab stops.
    let o8 = run(&["--tab-width=8", "--color=never", &f], &[]);
    let (src8, caret8) = echo_and_caret(&o8.stdout);
    assert_eq!(src8, "        echo $x");
    assert_eq!(caret8, caret);
}

#[test]
fn tab_width_without_tabs_is_unchanged() {
    let f = tmpfile("plain.sh", b"#!/bin/sh\necho $x\n");
    let a = run(&["--color=never", &f], &[]);
    let b = run(&["--tab-width=4", "--color=never", &f], &[]);
    assert_eq!(a.stdout, b.stdout);
}

#[test]
fn tab_width_rejects_bad_values() {
    let f = tmpfile("tabbed4.sh", TABBED);
    for bad in ["0", "x", ""] {
        let o = run(&[&format!("--tab-width={}", bad), &f], &[]);
        assert_eq!(o.code, 3, "{}: {}", bad, o.stderr);
        assert_eq!(o.stdout, "");
    }
}

#[test]
fn closed_stdout_is_quiet() {
    let f = tmpfile("pipe.sh", "echo $x $y\n".repeat(5000).as_bytes());
    for fmt in ["tty", "gcc", "json", "diff"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
            .args(["-f", fmt, &f, &f])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take()); // the reader goes away immediately
        let o = child.wait_with_output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(err, "", "{}", fmt);
        assert_eq!(o.status.code(), Some(1), "{}", fmt);
    }
}

#[test]
fn invalid_utf8_file_and_odd_locales_do_not_panic() {
    let f = tmpfile("latin.sh", b"#!/bin/sh\necho \"\xff\xfe\" $x\n");
    for env in [&[][..], &[("LC_ALL", "C")][..], &[("LANG", "ru_RU.KOI8-R")][..]] {
        for fmt in ["tty", "json", "diff", "gcc", "checkstyle"] {
            let o = run(&["-f", fmt, &f], env);
            assert_eq!(o.code, 1, "{} {:?}: {}", fmt, env, o.stderr);
            assert_eq!(o.stderr, "", "{} {:?}", fmt, env);
            assert!(!o.stdout.is_empty(), "{} {:?}", fmt, env);
        }
    }
}
