// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--fix` / `--dry-run`, an extension beyond upstream ShellCheck.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// A scratch directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("shelllint-fix-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, contents).unwrap();
        p
    }
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap()
    }
    fn mtime(&self, name: &str) -> SystemTime {
        std::fs::metadata(self.0.join(name)).unwrap().modified().unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_in(dir: &Path, args: &[&str], stdin: &str) -> Out {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let o = child.wait_with_output().unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// SC2086 (quote), SC2006 (backticks): both auto-fixable.
const FIXABLE: &str = "#!/bin/sh\necho $1\necho `date`\n";
const FIXED: &str = "#!/bin/sh\necho \"$1\"\necho $(date)\n";
/// SC2045 has no fix.
const UNFIXABLE: &str = "#!/bin/sh\nfor f in $(ls); do echo \"$f\"; done\n";
const CLEAN: &str = "#!/bin/sh\necho \"$1\"\n";

#[test]
fn fix_is_applied_and_clean_result_exits_0() {
    let d = TempDir::new("applied");
    d.write("t.sh", "#!/bin/sh\necho $1\n");
    let o = run_in(&d.0, &["--fix", "t.sh"], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(o.stdout, "");
    assert_eq!(o.stderr, "");
    assert_eq!(d.read("t.sh"), CLEAN);
}

#[test]
fn remaining_issues_are_reported_in_the_chosen_format() {
    let d = TempDir::new("remaining");
    d.write("t.sh", FIXABLE);
    let o = run_in(&d.0, &["--fix", "-f", "gcc", "t.sh"], "");
    assert_eq!(o.code, 1);
    assert_eq!(d.read("t.sh"), FIXED);
    // Only what is left after fixing: SC2046 and SC2005 on `echo $(date)`.
    assert!(!o.stdout.contains("SC2086") && !o.stdout.contains("SC2006"), "{}", o.stdout);
    assert!(o.stdout.contains("t.sh:3:6: warning:"), "{}", o.stdout);
    assert_eq!(o.stdout, run_in(&d.0, &["-f", "gcc", "t.sh"], "").stdout);

    // -f quiet prints nothing, but the exit code still says issues remain.
    let o = run_in(&d.0, &["--fix", "-f", "quiet", "t.sh"], "");
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (1, "", ""));
}

#[test]
fn second_run_is_a_no_op() {
    let d = TempDir::new("idempotent");
    d.write("t.sh", FIXABLE);
    let first = run_in(&d.0, &["--fix", "-f", "gcc", "t.sh"], "");
    let after_first = d.read("t.sh");
    assert_ne!(after_first, FIXABLE);
    let mtime = d.mtime("t.sh");
    let second = run_in(&d.0, &["--fix", "-f", "gcc", "t.sh"], "");
    assert_eq!(d.read("t.sh"), after_first);
    assert_eq!(d.mtime("t.sh"), mtime, "second --fix rewrote the file");
    assert_eq!((second.code, &second.stdout), (first.code, &first.stdout));
    // And nothing is left for -f diff to suggest.
    let diff = run_in(&d.0, &["-f", "diff", "t.sh"], "");
    assert!(!diff.stdout.contains("@@"), "{}", diff.stdout);
}

#[cfg(unix)]
#[test]
fn permissions_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let d = TempDir::new("perms");
    let p = d.write("t.sh", FIXABLE);
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o751)).unwrap();
    run_in(&d.0, &["--fix", "-f", "quiet", "t.sh"], "");
    assert_eq!(d.read("t.sh"), FIXED);
    let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode, 0o751);
    // No temporary file is left behind.
    let names: Vec<_> = std::fs::read_dir(&d.0).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("t.sh")]);
}

#[cfg(unix)]
#[test]
fn a_symlink_stays_a_symlink() {
    let d = TempDir::new("symlink");
    d.write("real.sh", FIXABLE);
    std::os::unix::fs::symlink("real.sh", d.0.join("link.sh")).unwrap();
    run_in(&d.0, &["--fix", "-f", "quiet", "link.sh"], "");
    assert!(std::fs::symlink_metadata(d.0.join("link.sh")).unwrap().file_type().is_symlink());
    assert_eq!(d.read("real.sh"), FIXED);
}

#[test]
fn crlf_line_endings_are_kept() {
    let d = TempDir::new("crlf");
    d.write("t.sh", "#!/bin/sh\r\necho $1\r\n");
    let o = run_in(&d.0, &["--fix", "-f", "gcc", "t.sh"], "");
    assert_eq!(d.read("t.sh"), "#!/bin/sh\r\necho \"$1\"\r\n");
    // SC1017 (literal carriage return) has no fix and remains.
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC1017"), "{}", o.stdout);
    let mtime = d.mtime("t.sh");
    run_in(&d.0, &["--fix", "-f", "quiet", "t.sh"], "");
    assert_eq!(d.mtime("t.sh"), mtime);
}

#[test]
fn missing_final_newline() {
    let d = TempDir::new("nonewline");
    // The last line is fixed: like patch, it gains a newline.
    d.write("a.sh", "#!/bin/sh\necho $1");
    // The last line is untouched: it stays without one.
    d.write("b.sh", "#!/bin/sh\necho $1\ntrue");
    let o = run_in(&d.0, &["--fix", "a.sh", "b.sh"], "");
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(d.read("a.sh"), "#!/bin/sh\necho \"$1\"\n");
    assert_eq!(d.read("b.sh"), "#!/bin/sh\necho \"$1\"\ntrue");
    let mtime = d.mtime("b.sh");
    run_in(&d.0, &["--fix", "a.sh", "b.sh"], "");
    assert_eq!(d.mtime("b.sh"), mtime);
}

#[test]
fn multiple_files_and_untouched_files() {
    let d = TempDir::new("multi");
    d.write("one.sh", FIXABLE);
    d.write("two.sh", "#!/bin/sh\necho $1\n");
    d.write("clean.sh", CLEAN);
    d.write("unfixable.sh", UNFIXABLE);
    let (clean_t, unfixable_t) = (d.mtime("clean.sh"), d.mtime("unfixable.sh"));
    let o = run_in(&d.0, &["--fix", "-f", "gcc", "one.sh", "two.sh", "clean.sh", "unfixable.sh"], "");
    assert_eq!(o.code, 1);
    assert_eq!(d.read("one.sh"), FIXED);
    assert_eq!(d.read("two.sh"), CLEAN);
    assert_eq!(d.read("clean.sh"), CLEAN);
    assert_eq!(d.read("unfixable.sh"), UNFIXABLE);
    assert_eq!(d.mtime("clean.sh"), clean_t, "a clean file was rewritten");
    assert_eq!(d.mtime("unfixable.sh"), unfixable_t, "a file with no fixes was rewritten");
    assert!(o.stdout.contains("unfixable.sh:2:10: error:"), "{}", o.stdout);
}

#[test]
fn agrees_with_diff_piped_to_patch() {
    if Command::new("patch").arg("--version").output().is_err() {
        eprintln!("patch not installed; skipping");
        return;
    }
    let scripts = [
        FIXABLE,
        "#!/bin/sh\necho $1",
        "#!/bin/sh\necho $1\ntrue",
        "#!/bin/sh\r\necho $1\r\n",
        "#!/bin/bash\nx=$(echo `id`)\n[ $x == 1 ] && echo $x\nfor i in 1 2 3; do\n\tread line\n\techo $line $i\ndone\n\necho \"$((  $x + 1 ))\"\ncd /tmp\n",
        "#!/bin/sh\na=1\n\n\n\n\n\n\n\n\necho $a\necho $b\n",
    ];
    for (i, script) in scripts.iter().enumerate() {
        let d = TempDir::new(&format!("patch{}", i));
        std::fs::create_dir_all(d.0.join("p")).unwrap();
        std::fs::create_dir_all(d.0.join("f")).unwrap();
        d.write("p/t.sh", script);
        d.write("f/t.sh", script);
        let diff = run_in(&d.0.join("p"), &["-f", "diff", "t.sh"], "");
        assert!(diff.stdout.contains("@@"), "script {} has no fixes", i);
        let mut patch = Command::new("patch")
            .args(["-p1", "-s", "--binary"])
            .current_dir(d.0.join("p"))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        use std::io::Write;
        patch.stdin.take().unwrap().write_all(diff.stdout.as_bytes()).unwrap();
        assert!(patch.wait().unwrap().success(), "patch failed on script {}", i);
        run_in(&d.0.join("f"), &["--fix", "-f", "quiet", "t.sh"], "");
        assert_eq!(d.read("f/t.sh"), d.read("p/t.sh"), "script {}: --fix differs from diff | patch", i);
    }
}

#[test]
fn stdin_is_skipped_with_an_error() {
    let d = TempDir::new("stdin");
    d.write("t.sh", "#!/bin/sh\necho $1\n");
    let o = run_in(&d.0, &["--fix", "-", "t.sh"], "echo $1\n");
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("cannot fix standard input"), "{}", o.stderr);
    assert_eq!(d.read("t.sh"), CLEAN);
    let o = run_in(&d.0, &["--fix", "-"], "echo $1\n");
    assert_eq!((o.code, o.stdout.as_str()), (2, ""));
}

#[test]
fn dry_run_prints_the_diff_and_writes_nothing() {
    let d = TempDir::new("dryrun");
    d.write("t.sh", FIXABLE);
    let mtime = d.mtime("t.sh");
    let o = run_in(&d.0, &["--fix", "--dry-run", "-f", "gcc", "t.sh"], "");
    let diff = run_in(&d.0, &["-f", "diff", "t.sh"], "");
    assert_eq!((o.code, &o.stdout, &o.stderr), (diff.code, &diff.stdout, &diff.stderr));
    assert!(o.stdout.starts_with("--- a/t.sh\n+++ b/t.sh\n"), "{}", o.stdout);
    assert_eq!(d.read("t.sh"), FIXABLE);
    assert_eq!(d.mtime("t.sh"), mtime);
    // --dry-run on its own is an error.
    let o = run_in(&d.0, &["--dry-run", "t.sh"], "");
    assert_eq!(o.code, 3);
    assert!(o.stderr.contains("--dry-run only works together with --fix"), "{}", o.stderr);
}

#[test]
fn sourced_files_are_fixed_only_when_given() {
    let d = TempDir::new("sourced");
    let main = "#!/bin/sh\n# shellcheck source=lib.sh\n. ./lib.sh\necho $1\n";
    d.write("main.sh", main);
    d.write("lib.sh", "echo $2\n");
    let o = run_in(&d.0, &["--fix", "-x", "-a", "-f", "gcc", "main.sh"], "");
    assert_eq!(d.read("lib.sh"), "echo $2\n");
    assert_eq!(d.read("main.sh"), main.replace("echo $1", "echo \"$1\""));
    assert!(o.stderr.contains("not fixing lib.sh: it was not given on the command line"), "{}", o.stderr);
    assert_eq!(o.code, 1);

    let o = run_in(&d.0, &["--fix", "-x", "-a", "-f", "gcc", "main.sh", "lib.sh"], "");
    assert_eq!(d.read("lib.sh"), "echo \"$2\"\n");
    assert_eq!(o.stderr, "");
}

#[test]
fn non_utf8_files_are_not_rewritten() {
    let d = TempDir::new("latin1");
    let bytes = b"#!/bin/sh\n# caf\xe9\necho $1\n";
    std::fs::write(d.0.join("t.sh"), bytes).unwrap();
    let o = run_in(&d.0, &["--fix", "-f", "gcc", "t.sh"], "");
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("not valid UTF-8"), "{}", o.stderr);
    assert_eq!(std::fs::read(d.0.join("t.sh")).unwrap(), bytes);
}
