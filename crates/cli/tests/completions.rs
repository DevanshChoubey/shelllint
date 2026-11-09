// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--completions=bash|zsh|fish` (an extension beyond ShellCheck).

use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn script(shell: &str) -> String {
    let (code, out, err) = run(&[&format!("--completions={}", shell)]);
    assert_eq!(code, 0, "{}", err);
    out
}

fn installed(shell: &str) -> bool {
    Command::new(shell).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

/// Feed `script` to `shell` with a no-execute flag; skip if it is not installed.
fn syntax_check(shell: &str, flag: &str, script: &str) {
    if !installed(shell) {
        eprintln!("skipping {} syntax check: {} is not installed", shell, shell);
        return;
    }
    let mut child = Command::new(shell)
        .arg(flag)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "{} {} failed: {}", shell, flag, String::from_utf8_lossy(&o.stderr));
}

const FLAGS: [&str; 6] = ["--severity", "--shell", "--format", "--exclude", "--external-sources", "--fix"];

#[test]
fn each_output_has_known_flags() {
    for shell in ["bash", "zsh", "fish"] {
        let s = script(shell);
        for f in FLAGS {
            assert!(s.contains(&f[2..]), "{} output lacks {}", shell, f);
        }
        assert!(s.contains("completions"), "{} output lacks --completions", shell);
    }
}

#[test]
fn bash_has_flags_and_values() {
    let s = script("bash");
    for f in FLAGS {
        assert!(s.contains(f), "missing {}", f);
    }
    assert!(s.contains("-s|--shell)"));
    assert!(s.contains("sh bash dash ksh busybox"));
    assert!(s.contains("checkstyle context diff gcc github json json1 quiet sarif tty"));
    assert!(s.contains("error warning info style"));
    assert!(s.contains("auto always never"));
}

#[test]
fn zsh_and_fish_have_values() {
    let z = script("zsh");
    assert!(z.starts_with("#compdef shelllint"));
    assert!(z.contains("(sh bash dash ksh busybox)"));
    assert!(z.contains("(error warning info style)"));
    let f = script("fish");
    assert!(f.contains("-l shell"));
    assert!(f.contains("-x -a 'sh bash dash ksh busybox'"));
    assert!(f.contains("-x -a 'error warning info style'"));
    assert!(f.contains("-a 'auto always never'"));
    assert!(f.contains("-s S -l severity"));
}

#[test]
fn bash_syntax() {
    syntax_check("bash", "-n", &script("bash"));
}

#[test]
fn zsh_syntax() {
    syntax_check("zsh", "-n", &script("zsh"));
}

#[test]
fn fish_syntax() {
    syntax_check("fish", "--no-execute", &script("fish"));
}

#[test]
fn unknown_shell_is_an_error() {
    let (code, out, err) = run(&["--completions=tcsh"]);
    assert_eq!(code, 3);
    assert!(out.is_empty());
    assert!(err.contains("Unknown shell"));
}
