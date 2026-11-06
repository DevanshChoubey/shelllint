// SPDX-License-Identifier: GPL-3.0-only
// Tests for the CLI, a port of ShellCheck shellcheck.hs (e11f964).
// Expected texts were captured from koalaman/shellcheck:latest.

use std::io::Write;
use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_with(args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust"));
    cmd.args(args)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let o = child.wait_with_output().unwrap();
    Out {
        code: o.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn run(args: &[&str], stdin: &str) -> Out {
    run_with(args, stdin, &[])
}

const CLEAN: &str = "#!/bin/sh\ntrue\n";
// Only the pilot checks exist so far; SC2148 (no shebang) is one of them.
const WARN: &str = "echo hi\n";

fn tmpfile(name: &str, contents: &str) -> String {
    let dir = std::env::temp_dir().join(format!("shellcheck-rust-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, contents).unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn help_matches_oracle() {
    let o = run(&["--help"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.stdout, include_str!("help.txt"));
}

#[test]
fn version_names_project_and_upstream() {
    let o = run(&["-V"], "");
    assert_eq!(o.code, 0);
    assert!(o.stdout.starts_with("shellcheck-rust"), "{}", o.stdout);
    assert!(o.stdout.contains("v0.11.0-171-ge11f964"), "{}", o.stdout);
    assert!(o.stdout.contains("license: GNU General Public License, version 3\n"));
    assert_eq!(run(&["--version"], "").stdout, o.stdout);
}

/// Every flag in `shellcheck --help` is accepted (exit 0 on a clean script
/// or an informational flag), in both short and long form.
#[test]
fn every_help_flag_is_accepted() {
    let help = include_str!("help.txt");
    let cases: &[(&str, &[&str])] = &[
        ("-a", &["-a", "-"]),
        ("--check-sourced", &["--check-sourced", "-"]),
        ("-C", &["-C", "-"]),
        ("--color", &["--color", "-"]),
        ("--color=never", &["--color=never", "-"]),
        ("-Calways", &["-Calways", "-"]),
        ("-i", &["-i", "SC2086", "-"]),
        ("--include", &["--include=SC2086", "-"]),
        ("-e", &["-e", "SC2086", "-"]),
        ("--exclude", &["--exclude=SC2086", "-"]),
        ("--extended-analysis", &["--extended-analysis=false", "-"]),
        ("-f", &["-f", "gcc", "-"]),
        ("--format", &["--format=json", "-"]),
        ("--norc", &["--norc", "-"]),
        ("--rcfile", &["--rcfile=/nonexistent-rc", "-"]),
        ("-o", &["-o", "all", "-"]),
        ("--enable", &["--enable=require-variable-braces", "-"]),
        ("-P", &["-P", "/tmp", "-"]),
        ("--source-path", &["--source-path=SCRIPTDIR", "-"]),
        ("-s", &["-s", "bash", "-"]),
        ("--shell", &["--shell=dash", "-"]),
        ("-S", &["-S", "error", "-"]),
        ("--severity", &["--severity=warning", "-"]),
        ("-W", &["-W", "1", "-"]),
        ("--wiki-link-count", &["--wiki-link-count=0", "-"]),
        ("-x", &["-x", "-"]),
        ("--external-sources", &["--external-sources", "-"]),
        ("--files-from", &["--files-from=/dev/null"]),
    ];
    for (flag, args) in cases {
        let o = run(args, CLEAN);
        assert_eq!(o.code, 0, "{} -> {:?}: {}", flag, args, o.stderr);
        assert!(!o.stderr.contains("unrecognized"), "{}: {}", flag, o.stderr);
    }
    for args in [&["-V"][..], &["--version"], &["--help"], &["--list-optional"]] {
        assert_eq!(run(args, "").code, 0, "{:?}", args);
    }
    // Make sure the table above covers every flag the oracle lists.
    for line in help.lines().skip(1) {
        for tok in line.split_whitespace().filter(|t| t.starts_with('-')) {
            let name = tok.split(['=', '[']).next().unwrap().trim_end_matches(',');
            assert!(
                cases.iter().any(|(f, a)| *f == name || a.iter().any(|x| x.starts_with(name)))
                    || ["-V", "--version", "--help", "--list-optional"].contains(&name),
                "flag {} not covered",
                name
            );
        }
    }
}

#[test]
fn exit_codes() {
    assert_eq!(run(&["-"], CLEAN).code, 0);
    let missing = run(&["/nonexistent/x.sh"], "");
    assert_eq!(missing.code, 2);
    assert_eq!(
        missing.stderr,
        "/nonexistent/x.sh: /nonexistent/x.sh: openBinaryFile: does not exist (No such file or directory)\n"
    );
    assert_eq!(run(&["--nonsense", "-"], CLEAN).code, 3);
    assert_eq!(run(&[], "").code, 3);
    assert_eq!(run(&["-e", "SCx", "-"], CLEAN).code, 3);
    assert_eq!(run(&["-S", "bad", "-"], CLEAN).code, 4);
    assert_eq!(run(&["--color=bad", "-"], CLEAN).code, 4);
    assert_eq!(run(&["-s", "zsh", "-"], CLEAN).code, 4);
    assert_eq!(run(&["-f", "nope", "-"], CLEAN).code, 4);
}

#[test]
fn exit_code_warning_is_1() {
    assert_eq!(run(&["-"], WARN).code, 1);
}

#[test]
fn bad_option_messages_match_oracle() {
    let usage = String::from_utf8(include_bytes!("help.txt").to_vec()).unwrap();
    let usage = format!("Usage: shellcheck [OPTIONS...] FILES...\n{}", usage.split_once('\n').unwrap().1);
    let o = run(&["--bogus"], "");
    assert_eq!(o.stderr, format!("unrecognized option `--bogus'\n\n{}\n", usage.trim_end_matches('\n').to_string() + "\n"));
    assert_eq!(run(&["-e"], "").stderr.lines().next().unwrap(), "option `-e' requires an argument CODE1,CODE2..");
    assert_eq!(
        run(&["--help=1"], "").stderr.lines().next().unwrap(),
        "option `--help' doesn't allow an argument"
    );
    let amb = run(&["--e"], "").stderr;
    assert!(amb.starts_with("option `--e' is ambiguous; could be one of:\n  -e CODE1,CODE2.."), "{}", amb);
    assert_eq!(run(&["-Cbad", "-"], "").stderr, "Unknown value for --color. Valid options are: auto, always, never\n");
    assert_eq!(
        run(&["-S", "x", "-"], "").stderr,
        "Unknown value for --severity. Valid options are: error, warning, info, style\n"
    );
    assert_eq!(run(&["-f", "nope", "-"], "").stderr, "Unknown format nope\nSupported formats:\n  checkstyle\n  diff\n  gcc\n  json\n  json1\n  quiet\n  tty\n");
    assert_eq!(run(&["-s", "zsh", "-"], "").stderr, "Unknown shell: zsh\n");
    assert_eq!(run(&["-W", "x", "-"], "").stderr, "Invalid number: x\n");
    assert_eq!(
        run(&["--extended-analysis=maybe", "-"], "").stderr,
        "Invalid boolean, expected true/false: maybe\n"
    );
}

#[test]
fn stdin_dash() {
    let o = run(&["-"], CLEAN);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.is_empty());
    // Not an "unrecognized option": a lone dash is a file operand.
    assert_eq!(run(&["-f", "checkstyle", "-"], CLEAN).stdout.lines().next().unwrap(), "<?xml version='1.0' encoding='UTF-8'?>");
}

#[test]
fn stdin_dash_reports_findings() {
    let o = run(&["-f", "gcc", "-"], WARN);
    assert_eq!(o.code, 1);
    assert!(o.stdout.starts_with("-:1:1: error: Tips depend on target shell and yours is unknown. Add a shebang or a 'shell' directive. [SC2148]"), "{}", o.stdout);
}

#[test]
fn combined_short_flags() {
    let o = run(&["-xa", "-"], CLEAN);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(run(&["-xae", "SC2086", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-xaeSC2086", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-xaCnever", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-xaq", "-"], CLEAN).code, 3);
    assert!(run(&["-xaq", "-"], CLEAN).stderr.starts_with("unrecognized option `-q'\n"));
    // -x -V behaves like -xV: version printed.
    assert!(run(&["-xV"], "").stdout.contains("version:"));
}

#[test]
fn include_exclude() {
    assert_eq!(run(&["-e", "SC2086,SC2034", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-i", "SC2086", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-e", "2086,,SC2034", "-i", "SC2086,2154", "-"], CLEAN).code, 0);
    assert_eq!(run(&["-e", "SC2086,bad", "-"], CLEAN).code, 3);
}

#[test]
fn include_exclude_behaviour() {
    assert_eq!(run(&["-e", "SC2148,SC2034", "-"], WARN).code, 0);
    assert_eq!(run(&["-i", "SC2148", "-"], WARN).code, 1);
    assert_eq!(run(&["-i", "SC2086", "-"], WARN).code, 0);
    assert_eq!(run(&["-i", "SC2034", "-"], WARN).code, 0);
}

#[test]
fn files_from() {
    let a = tmpfile("a.sh", CLEAN);
    let list = tmpfile("list.txt", &format!("# comment\n\n  {}  \n", a));
    assert_eq!(run(&[&format!("--files-from={}", list)], "").code, 0);
    assert_eq!(run(&["--files-from=-"], &format!("{}\n", a)).code, 0);
    let bad = run(&["--files-from=/nonexistent/list"], "");
    assert_eq!(bad.code, 2);
    assert_eq!(
        bad.stderr,
        "Could not read file list: /nonexistent/list: /nonexistent/list: openFile: does not exist (No such file or directory)\n"
    );
}

#[test]
fn shellcheck_opts_env_is_prepended() {
    let o = run_with(&["-"], CLEAN, &[("SHELLCHECK_OPTS", "-S  bad")]);
    assert_eq!(o.code, 4);
    let o = run_with(&["-"], CLEAN, &[("SHELLCHECK_OPTS", "--bogus")]);
    assert_eq!(o.code, 3);
}

#[test]
fn list_optional_format() {
    let o = run(&["--list-optional"], "");
    assert_eq!(o.code, 0);
    for line in o.stdout.lines().filter(|l| !l.is_empty()) {
        assert!(
            ["name:    ", "id:      ", "wiki:    ", "desc:    ", "example: ", "fix:     "]
                .iter()
                .any(|p| line.starts_with(p)),
            "{}",
            line
        );
    }
}

#[test]
fn quiet_stops_at_first_finding() {
    // A missing file makes the quiet formatter exit 1 immediately.
    let a = tmpfile("q.sh", CLEAN);
    let o = run(&["-f", "quiet", "/nonexistent/x.sh", &a], "");
    assert_eq!(o.code, 1);
    assert!(o.stdout.is_empty() && o.stderr.is_empty());
}

#[test]
fn on_failure_per_format() {
    let o = run(&["-f", "checkstyle", "/nonexistent/x.sh"], "");
    assert_eq!(o.code, 2);
    assert!(o.stdout.contains("<file name='/nonexistent/x.sh' >"), "{}", o.stdout);
    for fmt in ["gcc", "json", "json1", "tty"] {
        let o = run(&["-f", fmt, "-Cnever", "/nonexistent/x.sh"], "");
        assert_eq!(o.code, 2, "{}", fmt);
        assert!(o.stderr.starts_with("/nonexistent/x.sh: /nonexistent/x.sh: openBinaryFile"), "{}: {}", fmt, o.stderr);
    }
}

#[test]
fn missing_plus_clean_file_exits_2() {
    let a = tmpfile("two.sh", CLEAN);
    let o = run(&["/nonexistent/x.sh", &a], "");
    assert_eq!(o.code, 2);
    assert!(o.stderr.starts_with("/nonexistent/x.sh: "), "{}", o.stderr);
    let o = run(&[&a, "/nonexistent/x.sh"], "");
    assert_eq!(o.code, 2);
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shellcheck-rust-cli-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn editorconfig_directive_is_applied() {
    let d = tmpdir("ec-ok");
    std::fs::write(d.join(".editorconfig"), "root = true\n[*.sh]\nshellcheck.disable = SC2148\n").unwrap();
    let f = d.join("a.sh");
    std::fs::write(&f, WARN).unwrap();
    assert_eq!(run(&[f.to_str().unwrap()], "").code, 0);
    assert_eq!(run(&["--norc", f.to_str().unwrap()], "").code, 1);
}

#[test]
fn editorconfig_error_exits_4() {
    let d = tmpdir("ec-bad");
    std::fs::write(d.join(".editorconfig"), "root = maybe\n").unwrap();
    let f = d.join("a.sh");
    std::fs::write(&f, WARN).unwrap();
    let o = run(&["-f", "gcc", f.to_str().unwrap()], "");
    assert_eq!(o.code, 4);
    assert!(o.stdout.contains("Expected '=' after directive key. [SC1134]") || o.stdout.contains("[SC1134]"), "{}", o.stdout);
}
