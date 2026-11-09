// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `color=` / `wiki-link-count=` in .shellcheckrc and `--strict-rc`
// (extensions beyond ShellCheck).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_in(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// A fresh directory holding `t.sh` (SC2086) and a `.shellcheckrc`.
fn setup(name: &str, rc: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-rcstrict-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("t.sh"), "#!/bin/sh\necho $1\n").unwrap();
    std::fs::write(d.join("clean.sh"), "#!/bin/sh\ntrue\n").unwrap();
    std::fs::write(d.join(".shellcheckrc"), rc).unwrap();
    d
}

fn wiki_lines(s: &str) -> usize {
    s.lines().filter(|l| l.contains("/wiki/SC")).count()
}

#[test]
fn rc_color_is_applied_and_flag_wins() {
    let d = setup("color", "color=always\n");
    let o = run_in(&d, &["t.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stdout.contains("\x1b["), "{:?}", o.stdout);
    assert!(!o.stdout.contains("SC1107"), "{}", o.stdout);
    let o = run_in(&d, &["--color=never", "t.sh"]);
    assert!(!o.stdout.contains("\x1b["), "{:?}", o.stdout);
    // The same with --strict-rc: color= is a known key.
    let o = run_in(&d, &["--strict-rc", "t.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stdout.contains("\x1b["), "{:?}", o.stdout);
    assert_eq!(o.stderr, "");
    // Without the key, output is not coloured (stdout is a pipe).
    let d = setup("color-none", "");
    assert!(!run_in(&d, &["t.sh"]).stdout.contains("\x1b["));
    let d = setup("color-never", "color=never\n");
    assert!(!run_in(&d, &["t.sh"]).stdout.contains("\x1b["));
    assert!(run_in(&d, &["-C", "t.sh"]).stdout.contains("\x1b["));
}

#[test]
fn rc_wiki_link_count_is_applied_and_flag_wins() {
    let d = setup("wiki", "wiki-link-count=0\n");
    let base = setup("wiki-base", "");
    assert!(wiki_lines(&run_in(&base, &["t.sh"]).stdout) >= 1);
    let o = run_in(&d, &["t.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert_eq!(wiki_lines(&o.stdout), 0, "{}", o.stdout);
    assert!(!o.stdout.contains("SC1107"), "{}", o.stdout);
    let o = run_in(&d, &["-W", "1", "t.sh"]);
    assert_eq!(wiki_lines(&o.stdout), 1, "{}", o.stdout);
    let o = run_in(&d, &["--strict-rc", "t.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert_eq!(wiki_lines(&o.stdout), 0, "{}", o.stdout);
    // --norc ignores it.
    assert!(wiki_lines(&run_in(&d, &["--norc", "t.sh"]).stdout) >= 1);
}

#[test]
fn rc_extras_with_invalid_values_stay_unknown_directives() {
    let d = setup("badvals", "color=red\nwiki-link-count=x\n");
    let o = run_in(&d, &["-f", "gcc", "clean.sh"]);
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    let o = run_in(&d, &["--strict-rc", "-f", "gcc", "clean.sh"]);
    assert_eq!(o.code, 2);
    assert_eq!(o.stdout, "");
}

#[test]
fn unknown_key_without_and_with_strict_rc() {
    let d = setup("unknown", "frobnicate=1\n");
    let o = run_in(&d, &["-f", "gcc", "clean.sh"]);
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    let o = run_in(&d, &["--strict-rc", "-f", "gcc", "clean.sh"]);
    assert_eq!(o.code, 2, "{}", o.stdout);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("--strict-rc: ") && o.stderr.contains(".shellcheckrc:1:1: unknown directive"), "{}", o.stderr);
    // Not hidden by -S or -e.
    let o = run_in(&d, &["--strict-rc", "-S", "error", "-e", "1107", "clean.sh"]);
    assert_eq!(o.code, 2, "{}", o.stderr);
    // Ignored with --norc.
    let o = run_in(&d, &["--strict-rc", "--norc", "clean.sh"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
}

#[test]
fn invalid_shell_value_without_and_with_strict_rc() {
    let d = setup("shell", "shell=nosuchshell\n");
    let o = run_in(&d, &["-f", "gcc", "clean.sh"]);
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    let o = run_in(&d, &["--strict-rc", "-f", "gcc", "clean.sh"]);
    assert_eq!(o.code, 2);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("invalid shell value"), "{}", o.stderr);
    // A valid shell is fine under --strict-rc.
    let d = setup("shell-ok", "shell=bash\n");
    let o = run_in(&d, &["--strict-rc", "clean.sh"]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
}

#[test]
fn unreadable_rcfile_without_and_with_strict_rc() {
    let d = setup("rcfile", "");
    let o = run_in(&d, &["--rcfile=missing.rc", "clean.sh"]);
    assert_eq!(o.code, 0);
    assert_eq!(o.stderr, "Warning: unable to read --rcfile missing.rc\n");
    let o = run_in(&d, &["--strict-rc", "--rcfile=missing.rc", "clean.sh"]);
    assert_eq!(o.code, 2);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("--strict-rc: unable to read --rcfile missing.rc"), "{}", o.stderr);
    assert!(!o.stderr.contains("Warning:"), "{}", o.stderr);
    // A directory is not a readable rc file either.
    std::fs::create_dir_all(d.join("adir")).unwrap();
    assert_eq!(run_in(&d, &["--strict-rc", "--rcfile=adir", "clean.sh"]).code, 2);
    // A readable --rcfile passes, and its keys apply.
    std::fs::write(d.join("my.rc"), "color=always\n").unwrap();
    let o = run_in(&d, &["--strict-rc", "--rcfile=my.rc", "t.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stdout.contains("\x1b["));
}

#[test]
fn strict_rc_without_problems_changes_nothing() {
    let d = setup("clean", "disable=SC2086\nenable=all\n");
    let a = run_in(&d, &["-f", "gcc", "t.sh"]);
    let b = run_in(&d, &["--strict-rc", "-f", "gcc", "t.sh"]);
    assert_eq!((a.code, &a.stdout, &a.stderr), (b.code, &b.stdout, &b.stderr));
}
