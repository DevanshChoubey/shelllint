// SPDX-License-Identifier: GPL-3.0-or-later
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
    cmd.args(args).env_remove("SHELLCHECK_OPTS").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
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

/// Run with `dir` as the working directory and no stdin.
fn run_in(dir: &std::path::Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust"))
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

/// The options this port adds beyond ShellCheck, as `--help` lists them
/// after upstream's.
const EXTENSION_HELP: &str = concat!(
    "                      --fix                      Apply all auto-fixes to FILES in place, then report what is left\n",
    "                      --dry-run                  With --fix, print the fixes as a diff (like -f diff) instead\n",
);

/// `--help` as this port prints it: upstream's text (help.txt, verbatim)
/// with the extensions after upstream's options.
fn expected_help() -> String {
    let oracle = include_str!("help.txt");
    let body = oracle.strip_suffix('\n').unwrap();
    format!("{}{}\n", body, EXTENSION_HELP)
}

#[test]
fn help_matches_oracle() {
    let o = run(&["--help"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.stdout, expected_help());
}

#[test]
fn version_names_project_and_upstream() {
    let o = run(&["-V"], "");
    assert_eq!(o.code, 0);
    assert!(o.stdout.starts_with("shellcheck-rust"), "{}", o.stdout);
    assert!(o.stdout.contains("v0.11.0-171-ge11f964"), "{}", o.stdout);
    assert!(o.stdout.contains("license: GNU General Public License, version 3\n"));
    // The website line points at this project, not shellcheck.net.
    let website = o.stdout.lines().last().unwrap();
    assert_eq!(website, format!("website: {}", env!("CARGO_PKG_REPOSITORY")));
    assert!(!o.stdout.contains("shellcheck.net"), "{}", o.stdout);
    assert_eq!(o.stdout.lines().count(), 4, "{}", o.stdout);
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
        ("--fix", &["--fix", "--files-from=/dev/null"]),
        ("--dry-run", &["--fix", "--dry-run", "--files-from=/dev/null"]),
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
    for line in help.lines().skip(1).chain(EXTENSION_HELP.lines()) {
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
    let usage = expected_help();
    let usage = format!("Usage: shellcheck [OPTIONS...] FILES...\n{}", usage.split_once('\n').unwrap().1);
    let o = run(&["--bogus"], "");
    assert_eq!(
        o.stderr,
        format!("unrecognized option `--bogus'\n\n{}\n", usage.trim_end_matches('\n').to_string() + "\n")
    );
    assert_eq!(run(&["-e"], "").stderr.lines().next().unwrap(), "option `-e' requires an argument CODE1,CODE2..");
    assert_eq!(run(&["--help=1"], "").stderr.lines().next().unwrap(), "option `--help' doesn't allow an argument");
    let amb = run(&["--e"], "").stderr;
    assert!(amb.starts_with("option `--e' is ambiguous; could be one of:\n  -e CODE1,CODE2.."), "{}", amb);
    assert_eq!(run(&["-Cbad", "-"], "").stderr, "Unknown value for --color. Valid options are: auto, always, never\n");
    assert_eq!(
        run(&["-S", "x", "-"], "").stderr,
        "Unknown value for --severity. Valid options are: error, warning, info, style\n"
    );
    assert_eq!(
        run(&["-f", "nope", "-"], "").stderr,
        "Unknown format nope\nSupported formats:\n  checkstyle\n  diff\n  gcc\n  json\n  json1\n  quiet\n  tty\n"
    );
    assert_eq!(run(&["-s", "zsh", "-"], "").stderr, "Unknown shell: zsh\n");
    assert_eq!(run(&["-W", "x", "-"], "").stderr, "Invalid number: x\n");
    assert_eq!(run(&["--extended-analysis=maybe", "-"], "").stderr, "Invalid boolean, expected true/false: maybe\n");
}

#[test]
fn stdin_dash() {
    let o = run(&["-"], CLEAN);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.is_empty());
    // Not an "unrecognized option": a lone dash is a file operand.
    assert_eq!(
        run(&["-f", "checkstyle", "-"], CLEAN).stdout.lines().next().unwrap(),
        "<?xml version='1.0' encoding='UTF-8'?>"
    );
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
    assert!(
        o.stdout.contains("Expected '=' after directive key. [SC1134]") || o.stdout.contains("[SC1134]"),
        "{}",
        o.stdout
    );
}

// Each file group is rendered against its own file's contents
// (`siReadFile sys (Just True) (sourceFile group)` in TTY/GCC/JSON1/
// CheckStyle/Diff.hs), not the checked file's. Expected texts are the
// oracle's.
#[test]
fn sourced_file_groups_use_their_own_contents() {
    let d = tmpdir("groups-tty");
    std::fs::write(d.join("main.sh"), "#!/bin/sh\n. ./lib.sh\necho \"$y\"\n").unwrap();
    std::fs::write(d.join("lib.sh"), "#!/bin/sh\nfoo=1\necho $bar\n").unwrap();
    let o = run_in(&d, &["--norc", "-Cnever", "-x", "-a", "main.sh"]);
    assert_eq!(o.stdout, TTY_GROUPS);

    let d = tmpdir("groups-tab");
    std::fs::write(d.join("main.sh"), "#!/bin/sh\n. ./lib.sh\n").unwrap();
    std::fs::write(d.join("lib.sh"), "#!/bin/sh\n\t\techo $bar\n").unwrap();
    let o = run_in(&d, &["--norc", "-x", "-a", "-f", "gcc", "main.sh"]);
    assert_eq!(
        o.stdout,
        "./lib.sh:2:8: warning: bar is referenced but not assigned. [SC2154]\n\
         ./lib.sh:2:8: note: Double quote to prevent globbing and word splitting. [SC2086]\n"
    );
    let o = run_in(&d, &["--norc", "-x", "-a", "-f", "json1", "main.sh"]);
    assert!(o.stdout.contains("\"line\":2,\"endLine\":2,\"column\":8,\"endColumn\":12"), "{}", o.stdout);
    let o = run_in(&d, &["--norc", "-x", "-a", "-f", "checkstyle", "main.sh"]);
    assert!(o.stdout.contains("<error line='2' column='8' severity='warning'"), "{}", o.stdout);

    // diff reads lib.sh through the SystemInterface, so Latin-1 bytes decode.
    std::fs::write(d.join("lib.sh"), b"#!/bin/sh\n# \xe9\necho $bar\n").unwrap();
    let o = run_in(&d, &["--norc", "-x", "-a", "-f", "diff", "main.sh"]);
    assert_eq!(
        o.stdout,
        "--- a/./lib.sh\n+++ b/./lib.sh\n@@ -1,3 +1,3 @@\n #!/bin/sh\n # \u{e9}\n-echo $bar\n+echo \"$bar\"\n\n"
    );
}

const TTY_GROUPS: &str = "\nIn ./lib.sh line 2:\nfoo=1\n^-^ SC2034 (warning): foo appears unused. Verify use (or export if used externally).\n\n\nIn ./lib.sh line 3:\necho $bar\n     ^--^ SC2154 (warning): bar is referenced but not assigned.\n     ^--^ SC2086 (info): Double quote to prevent globbing and word splitting.\n\nDid you mean:\necho \"$bar\"\n\n\nIn main.sh line 3:\necho \"$y\"\n      ^-- SC2154 (warning): y is referenced but not assigned.\n\nFor more information:\n  https://www.shellcheck.net/wiki/SC2034 -- foo appears unused. Verify use (o...\n  https://www.shellcheck.net/wiki/SC2154 -- bar is referenced but not assigned.\n  https://www.shellcheck.net/wiki/SC2086 -- Double quote to prevent globbing ...\n";

// The CLI decodes input with shellcheck_checker::decode_string: a UTF-8
// encoded surrogate is one character, as Haskell's `chr` makes it, so later
// columns match the oracle's (2:10, not 2:12).
#[test]
fn encoded_surrogate_is_one_column() {
    let d = tmpdir("surrogate");
    std::fs::write(d.join("s.sh"), b"#!/bin/sh\necho \"\xed\xa0\x80\" $x\n").unwrap();
    let o = run_in(&d, &["--norc", "-f", "gcc", "s.sh"]);
    assert_eq!(
        o.stdout,
        "s.sh:2:10: warning: x is referenced but not assigned. [SC2154]\n\
         s.sh:2:10: note: Double quote to prevent globbing and word splitting. [SC2086]\n"
    );
}

// stdin can be read once: `getContents` (--files-from=-) and `inputFile "-"`
// close it, so a later read fails as in the oracle. Failed reads are not
// cached, so each later '-' fails.
#[test]
fn stdin_is_read_only_once() {
    let a = tmpfile("once.sh", CLEAN);
    let o = run(&["--norc", "-f", "gcc", "--files-from=-"], &format!("{}\n-\n", a));
    assert_eq!(o.code, 2);
    assert_eq!(o.stdout, "");
    assert_eq!(o.stderr, "-: <stdin>: hGetContents: illegal operation (handle is closed)\n");
    let o = run(&["--norc", "-f", "gcc", "--files-from=-", "-", "-"], &format!("{}\n", a));
    assert_eq!(o.code, 2);
    assert_eq!(o.stderr, "-: <stdin>: hGetContents: illegal operation (handle is closed)\n".repeat(2));
    // A second getContents is an uncaught exception.
    let o = run(&["--norc", "-f", "gcc", "--files-from=-", "--files-from=-"], &format!("{}\n", a));
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, "");
    assert_eq!(o.stderr, "shellcheck-rust: <stdin>: hGetContents: illegal operation (handle is semi-closed)\n");
    // Reading '-' twice as a script uses the cached contents.
    let o = run(&["--norc", "-f", "gcc", "-", "-"], "#!/bin/sh\necho $x\n");
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout.matches("[SC2154]").count(), 2, "{}", o.stdout);
}

// An internal error (here regex-tdfa rejecting an EditorConfig glob, a
// Haskell `error`) prints `<prog>: <message>` and exits 1, not 101.
#[test]
fn internal_error_exits_1() {
    let d = tmpdir("ec-regex");
    std::fs::write(d.join(".editorconfig"), "root=true\n[[z-a].sh]\nshellcheck.disable=SC2086\n").unwrap();
    std::fs::write(d.join("a.sh"), "#!/bin/sh\ntrue\n").unwrap();
    let o = run_in(&d, &["-f", "gcc", "a.sh"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.starts_with("shellcheck-rust: "), "{}", o.stderr);
    assert!(!o.stderr.contains("panicked") && !o.stderr.contains("RUST_BACKTRACE"), "{}", o.stderr);
}

// --rcfile accepts anything `doesFileExist` does, such as a fifo (or a
// process substitution's /proc/self/fd/N). Oracle: SC2148 and SC2154 only.
#[test]
fn rcfile_may_be_a_fifo() {
    let d = tmpdir("rc-fifo");
    let fifo = d.join("p");
    let _ = std::fs::remove_file(&fifo);
    assert!(Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    std::fs::write(d.join("t.sh"), "echo $x\n").unwrap();
    let fifo2 = fifo.clone();
    let writer = std::thread::spawn(move || std::fs::write(&fifo2, "disable=SC2086\n").unwrap());
    let o = run_in(&d, &["-f", "gcc", "--rcfile", fifo.to_str().unwrap(), "t.sh"]);
    // If the fifo was never opened, unblock the writer instead of hanging.
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _r = std::fs::OpenOptions::new().read(true).custom_flags(0o4000 /* O_NONBLOCK */).open(&fifo);
        writer.join().unwrap();
    }
    assert_eq!(o.stderr, "");
    assert_eq!(
        o.stdout,
        "t.sh:1:1: error: Tips depend on target shell and yours is unknown. Add a shebang or a 'shell' directive. [SC2148]\n\
         t.sh:1:6: warning: x is referenced but not assigned. [SC2154]\n"
    );
}

// --list-optional is `sortOn cdName ShellCheck.Analyzer.optionalChecks`,
// with shellcheck_checker::optional_checks as the one source of truth.
#[test]
fn list_optional_is_checker_optional_checks() {
    let o = run(&["--list-optional"], "");
    let mut names: Vec<String> = shellcheck_checker::optional_checks().into_iter().map(|c| c.name).collect();
    names.sort();
    let listed: Vec<&str> = o.stdout.lines().filter_map(|l| l.strip_prefix("name:    ")).collect();
    assert_eq!(listed, names);
}

/// Like `run_in`, with extra environment variables.
fn run_in_env(dir: &std::path::Path, args: &[&str], env: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust"));
    cmd.args(args).current_dir(dir).env_remove("SHELLCHECK_OPTS").stdin(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

#[test]
fn relative_or_empty_xdg_config_home_falls_back_to_home_config() {
    // Expected output captured from the Haskell oracle.
    let d = tmpdir("xdg-rel");
    let (w, h) = (d.join("w"), d.join("h"));
    std::fs::create_dir_all(w.join("xdg")).unwrap();
    std::fs::create_dir_all(h.join(".config")).unwrap();
    std::fs::write(w.join("a.sh"), "#!/bin/sh\necho $1\nfoo=`ls`\n").unwrap();
    std::fs::write(w.join("xdg/shellcheckrc"), "disable=SC2086\n").unwrap();
    std::fs::write(w.join("xdg/editorconfig.ini"), "[*]\nshellcheck.disable=SC2006\n").unwrap();
    std::fs::write(h.join(".config/shellcheckrc"), "disable=SC2034\n").unwrap();
    let home = h.to_string_lossy().into_owned();
    let home_only = "a.sh:2:6: note: Double quote to prevent globbing and word splitting. [SC2086]\n\
                     a.sh:3:5: note: Use $(...) notation instead of legacy backticks `...`. [SC2006]\n";
    for xdg in ["xdg", "./xdg", "xdg/", "../w/xdg", ""] {
        let o = run_in_env(&w, &["-f", "gcc", "a.sh"], &[("HOME", &home), ("XDG_CONFIG_HOME", xdg)]);
        assert_eq!(o.stdout, home_only, "XDG_CONFIG_HOME={:?}", xdg);
        assert_eq!(o.code, 1);
    }
    // An absolute one is used, and its editorconfig.ini too.
    let abs = format!("{}/./", w.join("xdg").to_string_lossy());
    let o = run_in_env(&w, &["-f", "gcc", "a.sh"], &[("HOME", &home), ("XDG_CONFIG_HOME", &abs)]);
    assert_eq!(o.stdout, "a.sh:3:1: warning: foo appears unused. Verify use (or export if used externally). [SC2034]\n");
}

#[cfg(unix)]
#[test]
fn file_open_errors_use_ghc_io_error_text() {
    let d = std::env::temp_dir().join(format!("sc-ioerr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("ok.sh"), CLEAN).unwrap();
    std::os::unix::fs::symlink("l1", d.join("l2")).unwrap();
    std::os::unix::fs::symlink("l2", d.join("l1")).unwrap();
    let long = format!("{}.sh", "0".repeat(300));
    let cases = [
        ("ok.sh/x", "inappropriate type (Not a directory)"),
        ("l1", "invalid argument (Symbolic link loop)"),
        (long.as_str(), "invalid argument (Filename too long)"),
    ];
    for (f, desc) in cases {
        let o = run_in(&d, &["--norc", f]);
        assert_eq!(o.code, 2, "{f}");
        assert_eq!(o.stderr, format!("{f}: {f}: openBinaryFile: {desc}\n"));
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(target_os = "linux")]
#[test]
fn read_error_after_open_is_reported_by_hgetcontents() {
    let o = run(&["--norc", "/proc/self/mem"], "");
    assert_eq!(o.code, 2);
    assert_eq!(o.stderr, "/proc/self/mem: /proc/self/mem: hGetContents: hardware fault (I/O error)\n");
}

#[test]
fn files_from_trims_with_ghc_is_space() {
    let d = std::env::temp_dir().join(format!("sc-filesfrom-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.sh"), CLEAN).unwrap();
    // U+00A0 and U+3000 are isSpace; U+0085 and U+2028 are not, so they stay in the name.
    std::fs::write(d.join("list"), "\u{a0}a.sh\u{3000}\n\u{85}b.sh\n").unwrap();
    let o = run_in(&d, &["--norc", "--files-from", "list"]);
    assert_eq!(o.code, 2);
    assert_eq!(o.stderr, "\u{85}b.sh: \u{85}b.sh: openBinaryFile: does not exist (No such file or directory)\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// GHC's RTS prints "out of memory" and exits 251; the CLI's allocator wrapper does the same.
#[cfg(target_os = "linux")]
#[test]
fn allocation_failure_exits_251_with_message() {
    let d = std::env::temp_dir().join(format!("sc-oom-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    // A sparse 3 GB file cannot be read into memory under a 1.5 GB address-space limit.
    std::fs::File::create(d.join("big.sh")).unwrap().set_len(3 << 30).unwrap();
    let o = Command::new("sh")
        .arg("-c")
        .arg("ulimit -v 1500000 && exec \"$0\" --norc big.sh")
        .arg(env!("CARGO_BIN_EXE_shellcheck-rust"))
        .current_dir(&d)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(o.status.code(), Some(251));
    assert_eq!(String::from_utf8_lossy(&o.stderr), "shellcheck-rust: out of memory\n");
}
