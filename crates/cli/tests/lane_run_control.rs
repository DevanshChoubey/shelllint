// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--summary` and `--fail-fast`, extensions beyond upstream ShellCheck.

use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("shelllint-rc-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.0.join(name), contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_in(d: &TempDir, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(&d.0)
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

const CLEAN: &str = "#!/bin/sh\necho \"$1\"\n";
/// SC2086 (info).
const INFO: &str = "#!/bin/sh\necho $1\n";
/// SC2034 (warning): unused variable.
const WARNING: &str = "#!/bin/sh\nx=1\n";
/// SC2148 (error): no shebang.
const ERROR: &str = "echo \"$1\"\n";

fn gcc_lines(o: &Out) -> usize {
    o.stdout.lines().count()
}

/// Brackets and braces balance outside strings and the text is one value.
fn looks_like_json(s: &str) -> bool {
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for c in s.trim().chars() {
        if in_str {
            match (esc, c) {
                (true, _) => esc = false,
                (false, '\\') => esc = true,
                (false, '"') => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    !in_str && depth == 0 && (s.trim().starts_with('{') || s.trim().starts_with('['))
}

#[test]
fn summary_goes_to_stderr_and_leaves_stdout_alone() {
    let d = TempDir::new("summary");
    d.write("clean.sh", CLEAN);
    d.write("info.sh", INFO);
    d.write("warn.sh", WARNING);
    d.write("err.sh", ERROR);
    let files = ["clean.sh", "info.sh", "warn.sh", "err.sh"];
    for fmt in ["tty", "gcc", "json", "json1", "checkstyle", "diff", "quiet"] {
        let mut a = vec!["-f", fmt, "--color=never"];
        a.extend(files);
        let plain = run_in(&d, &a);
        a.push("--summary");
        let with = run_in(&d, &a);
        assert_eq!(with.stdout, plain.stdout, "stdout differs for {}", fmt);
        assert_eq!(with.code, plain.code, "exit code differs for {}", fmt);
        assert_eq!(plain.stderr, "", "{}", fmt);
        if fmt != "quiet" {
            assert_eq!(with.stderr, "4 file(s) checked: 1 error(s), 1 warning(s), 1 info(s), 0 style(s)\n", "{}", fmt);
        }
    }
}

#[test]
fn summary_on_clean_input_and_with_severity() {
    let d = TempDir::new("summary-clean");
    d.write("clean.sh", CLEAN);
    d.write("info.sh", INFO);
    let o = run_in(&d, &["--summary", "clean.sh"]);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
    assert_eq!(o.stderr, "1 file(s) checked: 0 error(s), 0 warning(s), 0 info(s), 0 style(s)\n");
    // Findings below the active severity are not counted.
    let o = run_in(&d, &["--summary", "-S", "warning", "info.sh"]);
    assert_eq!(o.code, 0);
    assert_eq!(o.stderr, "1 file(s) checked: 0 error(s), 0 warning(s), 0 info(s), 0 style(s)\n");
}

#[test]
fn without_flags_nothing_changes() {
    let d = TempDir::new("noflags");
    d.write("a.sh", INFO);
    d.write("b.sh", INFO);
    let o = run_in(&d, &["-f", "gcc", "a.sh", "b.sh"]);
    assert_eq!(o.code, 1);
    assert_eq!(gcc_lines(&o), 2);
    assert_eq!(o.stderr, "");
}

#[test]
fn fail_fast_stops_after_first_file_with_findings() {
    let d = TempDir::new("failfast");
    d.write("clean.sh", CLEAN);
    d.write("a.sh", INFO);
    d.write("b.sh", ERROR);
    let all = run_in(&d, &["-f", "gcc", "clean.sh", "a.sh", "b.sh"]);
    assert_eq!(all.code, 1);
    assert_eq!(gcc_lines(&all), 2);
    let ff = run_in(&d, &["-f", "gcc", "--fail-fast", "clean.sh", "a.sh", "b.sh"]);
    assert_eq!(ff.code, 1);
    assert_eq!(ff.stdout.lines().count(), 1, "{}", ff.stdout);
    assert!(ff.stdout.starts_with("a.sh:"), "{}", ff.stdout);
    assert_eq!(ff.stderr, "");
    // The exit code is that of the first failing file: an error-only second
    // file does not matter, and a clean run is unchanged.
    let clean = run_in(&d, &["--fail-fast", "clean.sh", "clean.sh"]);
    assert_eq!((clean.code, clean.stdout.as_str()), (0, ""));
}

#[test]
fn fail_fast_uses_the_active_severity() {
    let d = TempDir::new("failfast-sev");
    d.write("a.sh", INFO);
    d.write("b.sh", ERROR);
    // At -S warning the info in a.sh is not a finding, so b.sh is reached.
    let o = run_in(&d, &["-f", "gcc", "-S", "warning", "--fail-fast", "a.sh", "b.sh"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout.lines().count(), 1, "{}", o.stdout);
    assert!(o.stdout.starts_with("b.sh:"), "{}", o.stdout);
}

#[test]
fn fail_fast_json1_stays_parseable() {
    let d = TempDir::new("failfast-json1");
    d.write("a.sh", INFO);
    d.write("b.sh", INFO);
    for fmt in ["json1", "json", "checkstyle"] {
        let o = run_in(&d, &["-f", fmt, "--fail-fast", "a.sh", "b.sh"]);
        assert_eq!(o.code, 1, "{}", fmt);
        assert!(o.stdout.contains("a.sh"), "{}", o.stdout);
        assert!(!o.stdout.contains("b.sh"), "{}: {}", fmt, o.stdout);
        if fmt != "checkstyle" {
            assert!(looks_like_json(&o.stdout), "{}: {}", fmt, o.stdout);
        } else {
            assert!(o.stdout.trim_end().ends_with("</checkstyle>"), "{}", o.stdout);
        }
    }
}

#[test]
fn fail_fast_with_summary_counts_processed_files_only() {
    let d = TempDir::new("both");
    d.write("clean.sh", CLEAN);
    d.write("a.sh", INFO);
    d.write("b.sh", ERROR);
    let o = run_in(&d, &["--fail-fast", "--summary", "clean.sh", "a.sh", "b.sh"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stderr, "2 file(s) checked: 0 error(s), 0 warning(s), 1 info(s), 0 style(s)\n");
}
