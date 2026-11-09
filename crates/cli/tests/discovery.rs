// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `-r`, `--exclude-path` and `--changed-since` (extensions beyond
// upstream ShellCheck).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn tmp() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("shelllint-discovery-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
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

fn put(dir: &Path, rel: &str, contents: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, contents).unwrap();
}

/// A script with exactly one finding (SC2086) on line 3.
const BAD: &str = "#!/bin/sh\nx=$(date)\necho $x\n";

/// `-f gcc` lines reduced to their file names, in output order.
fn gcc_files(o: &Out) -> Vec<String> {
    o.stdout.lines().map(|l| l.split(':').next().unwrap().to_string()).collect()
}

#[test]
fn walk_is_sorted_and_deterministic() {
    let d = tmp();
    put(&d, "t/b.sh", BAD);
    put(&d, "t/a.sh", BAD);
    put(&d, "t/sub/z.bash", BAD);
    put(&d, "t/sub/c.ksh", BAD);
    put(&d, "t/notes.txt", "hi\n");
    let o = run_in(&d, &["-r", "-f", "gcc", "t"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert_eq!(gcc_files(&o), ["t/a.sh", "t/b.sh", "t/sub/c.ksh", "t/sub/z.bash"]);
    let again = run_in(&d, &["-r", "-f", "gcc", "t"]);
    assert_eq!(o.stdout, again.stdout);
}

#[test]
fn directory_without_flag_is_unchanged() {
    let d = tmp();
    put(&d, "t/a.sh", BAD);
    let o = run_in(&d, &["-f", "gcc", "t"]);
    assert_ne!(o.code, 0);
    assert!(!o.stdout.contains("a.sh"), "{}", o.stdout);
}

#[test]
fn skips_vcs_and_node_modules() {
    let d = tmp();
    put(&d, "t/keep.sh", BAD);
    put(&d, "t/node_modules/dep/x.sh", BAD);
    put(&d, "t/.git/hooks/pre-commit.sh", BAD);
    put(&d, "t/.hg/x.sh", BAD);
    put(&d, "t/.svn/x.sh", BAD);
    let o = run_in(&d, &["-r", "-f", "gcc", "t"]);
    assert_eq!(gcc_files(&o), ["t/keep.sh"]);
}

#[test]
fn detects_shebang_without_extension() {
    let d = tmp();
    put(&d, "t/a", "#!/bin/bash\nx=$(date)\necho $x\n");
    put(&d, "t/b", "#!/usr/bin/env sh\nx=$(date)\necho $x\n");
    put(&d, "t/c", "#!/usr/bin/env -S bash -e\nx=$(date)\necho $x\n");
    put(&d, "t/py", "#!/usr/bin/python\nprint(1)\n");
    put(&d, "t/plain", "x=$(date)\necho $x\n");
    let o = run_in(&d, &["-r", "-f", "gcc", "t"]);
    assert_eq!(gcc_files(&o), ["t/a", "t/b", "t/c"]);
}

#[test]
fn explicit_files_are_checked_without_extension() {
    let d = tmp();
    put(&d, "plain", "# shellcheck shell=sh\nx=$(date)\necho $x\n");
    let o = run_in(&d, &["-f", "gcc", "plain"]);
    assert_eq!(gcc_files(&o), ["plain"]);
    let o = run_in(&d, &["-r", "-f", "gcc", "plain"]);
    assert_eq!(gcc_files(&o), ["plain"]);
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    let d = tmp();
    put(&d, "t/real.sh", BAD);
    put(&d, "other/x.sh", BAD);
    std::os::unix::fs::symlink(d.join("other"), d.join("t/linkdir")).unwrap();
    std::os::unix::fs::symlink(d.join("t"), d.join("t/cycle")).unwrap();
    std::os::unix::fs::symlink(d.join("other/x.sh"), d.join("t/linkfile.sh")).unwrap();
    let o = run_in(&d, &["-r", "-f", "gcc", "t"]);
    assert_eq!(gcc_files(&o), ["t/real.sh"]);
}

#[test]
fn empty_walk_is_not_an_error() {
    let d = tmp();
    put(&d, "t/readme.txt", "hi\n");
    let o = run_in(&d, &["-r", "t"]);
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    let o = run_in(&d, &["-r", "-f", "json", "t"]);
    assert_eq!((o.code, o.stdout.trim()), (0, "[]"));
}

#[test]
fn exclude_path_applies_to_walked_and_explicit_files() {
    let d = tmp();
    put(&d, "t/a.sh", BAD);
    put(&d, "t/foo.generated.sh", BAD);
    put(&d, "t/deep/vendor/b.sh", BAD);
    put(&d, "t/deep/c.sh", BAD);
    let o = run_in(&d, &["-r", "-f", "gcc", "--exclude-path=*.generated.sh", "--exclude-path=vendor", "t"]);
    assert_eq!(gcc_files(&o), ["t/a.sh", "t/deep/c.sh"]);
    let o = run_in(&d, &["-f", "gcc", "--exclude-path=*.generated.sh", "t/a.sh", "t/foo.generated.sh"]);
    assert_eq!(gcc_files(&o), ["t/a.sh"]);
    let o = run_in(&d, &["-f", "gcc", "--exclude-path=t/deep/**", "t/a.sh", "t/deep/c.sh"]);
    assert_eq!(gcc_files(&o), ["t/a.sh"]);
}

#[test]
fn everything_excluded() {
    let d = tmp();
    put(&d, "t/a.sh", BAD);
    let o = run_in(&d, &["-r", "--exclude-path=*.sh", "."]);
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
    // Without a walk, excluding every explicit file is the empty-list error.
    let o = run_in(&d, &["--exclude-path=*.sh", "t/a.sh"]);
    assert_eq!(o.code, 3);
    assert!(o.stderr.contains("No files specified"), "{}", o.stderr);
}

// ---- --changed-since ----

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(o.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&o.stderr));
}

fn repo() -> PathBuf {
    let d = tmp();
    git(&d, &["init", "-q"]);
    git(&d, &["config", "user.name", "Test"]);
    git(&d, &["config", "user.email", "test@example.com"]);
    git(&d, &["config", "commit.gpgsign", "false"]);
    d
}

fn commit_all(d: &Path) {
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "c"]);
}

#[test]
fn changed_since_keeps_only_changed_lines() {
    let d = repo();
    // Findings on lines 2 and 4.
    put(&d, "a.sh", "#!/bin/sh\na=$(date)\nb=$(date)\necho $a\necho ok\necho $b\n");
    commit_all(&d);
    let o = run_in(&d, &["-f", "gcc", "a.sh"]);
    assert_eq!(o.stdout.lines().count(), 2);
    // Nothing changed: no findings, exit 0.
    let o = run_in(&d, &["-f", "gcc", "--changed-since", "a.sh"]);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""), "{}", o.stderr);
    // Touch line 4 only.
    put(&d, "a.sh", "#!/bin/sh\na=$(date)\nb=$(date)\necho $a\necho ok\necho $b # x\n");
    for fmt in ["gcc", "json", "json1", "checkstyle", "tty", "diff", "quiet"] {
        let o = run_in(&d, &["-f", fmt, "--changed-since=HEAD", "a.sh"]);
        assert_ne!(o.code, 2, "{fmt}: {}", o.stderr);
    }
    let o = run_in(&d, &["-f", "gcc", "--changed-since=HEAD", "a.sh"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout.lines().count(), 1, "{}", o.stdout);
    assert!(o.stdout.starts_with("a.sh:6:"), "{}", o.stdout);
    // A pure deletion marks no lines.
    put(&d, "a.sh", "#!/bin/sh\na=$(date)\nb=$(date)\necho $a\necho $b\n");
    let o = run_in(&d, &["-f", "gcc", "--changed-since", "a.sh"]);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""), "{}", o.stderr);
}

#[test]
fn changed_since_untracked_file_is_all_changed() {
    let d = repo();
    put(&d, "old.sh", "#!/bin/sh\ntrue\n");
    commit_all(&d);
    put(&d, "new.sh", "#!/bin/sh\na=$(date)\necho $a\necho $a\n");
    let o = run_in(&d, &["-f", "gcc", "--changed-since", "new.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert_eq!(o.stdout.lines().count(), 2);
}

#[test]
fn changed_since_against_an_older_ref() {
    let d = repo();
    put(&d, "a.sh", "#!/bin/sh\na=$(date)\necho $a\n");
    commit_all(&d);
    put(&d, "a.sh", "#!/bin/sh\na=$(date)\necho $a\necho $a\n");
    commit_all(&d);
    let o = run_in(&d, &["-f", "gcc", "--changed-since=HEAD", "a.sh"]);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
    let o = run_in(&d, &["-f", "gcc", "--changed-since=HEAD~1", "a.sh"]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.starts_with("a.sh:4:"), "{}", o.stdout);
}

#[test]
fn changed_since_bad_ref_and_non_repo_and_stdin() {
    let d = repo();
    put(&d, "a.sh", "#!/bin/sh\ntrue\n");
    commit_all(&d);
    let o = run_in(&d, &["--changed-since=no-such-ref", "a.sh"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("unknown git revision 'no-such-ref'"), "{}", o.stderr);
    assert_eq!(o.stdout, "");

    let plain = tmp();
    put(&plain, "a.sh", "#!/bin/sh\ntrue\n");
    let o = run_in(&plain, &["--changed-since", "a.sh"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("not in a git repository"), "{}", o.stderr);

    let o = run_in(&d, &["--changed-since", "-"]);
    assert_eq!(o.code, 3);
    assert!(o.stderr.contains("standard input"), "{}", o.stderr);
}
