// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only test support (no ShellCheck counterpart).
#![allow(dead_code)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// One reported finding.
#[derive(Debug, Clone)]
pub struct Diag {
    pub code: u32,
    pub line: u32,
    pub message: String,
}

fn parse(stdout: &[u8]) -> Vec<Diag> {
    let v: serde_json::Value = serde_json::from_slice(stdout).expect("json1 output");
    v["comments"]
        .as_array()
        .expect("comments array")
        .iter()
        .map(|c| Diag {
            code: c["code"].as_u64().unwrap() as u32,
            line: c["line"].as_u64().unwrap() as u32,
            message: c["message"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

fn base_cmd(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shelllint"));
    cmd.args(["--norc", "-f", "json1"]).args(args).env_remove("SHELLCHECK_OPTS");
    cmd
}

fn finish(o: std::process::Output, what: &str) -> Vec<Diag> {
    assert!(
        matches!(o.status.code(), Some(0) | Some(1)),
        "shelllint {what} exited {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    parse(&o.stdout)
}

/// Lint `script` from stdin with the given extra arguments.
pub fn lint(args: &[&str], script: &str) -> Vec<Diag> {
    let mut child =
        base_cmd(args).arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    finish(o, &format!("{args:?}"))
}

/// Lint under `--shell=zsh`.
pub fn zsh(script: &str) -> Vec<Diag> {
    lint(&["--shell=zsh"], script)
}

/// Lint under `--shell=zsh` with extra arguments (for example `--enable=...`).
pub fn zsh_with(args: &[&str], script: &str) -> Vec<Diag> {
    let mut a = vec!["--shell=zsh"];
    a.extend_from_slice(args);
    lint(&a, script)
}

pub fn shell(name: &str, script: &str) -> Vec<Diag> {
    lint(&[&format!("--shell={name}")], script)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Write `script` to a fresh temp directory under the given file name and lint that path.
/// The name decides the shell when no flag is given (`plugin.zsh`).
pub fn lint_named(file_name: &str, args: &[&str], script: &str) -> Vec<Diag> {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "shelllint-zsh-acc-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(file_name);
    std::fs::write(&path, script).unwrap();
    let o = base_cmd(args).arg(&path).stdout(Stdio::piped()).stderr(Stdio::piped()).output().unwrap();
    let r = finish(o, &format!("{args:?} {file_name}"));
    let _ = std::fs::remove_dir_all(&dir);
    r
}

pub fn codes(d: &[Diag]) -> Vec<u32> {
    d.iter().map(|x| x.code).collect()
}

pub fn pairs(d: &[Diag]) -> Vec<(u32, u32)> {
    d.iter().map(|x| (x.code, x.line)).collect()
}

/// Lines (in report order) at which `code` is reported.
pub fn lines(d: &[Diag], code: u32) -> Vec<u32> {
    d.iter().filter(|x| x.code == code).map(|x| x.line).collect()
}

pub fn has(d: &[Diag], code: u32) -> bool {
    d.iter().any(|x| x.code == code)
}

pub fn messages(d: &[Diag], code: u32) -> Vec<String> {
    d.iter().filter(|x| x.code == code).map(|x| x.message.clone()).collect()
}

/// Any SC1xxx (parse-level) finding.
pub fn parse_level(d: &[Diag]) -> Vec<(u32, u32)> {
    pairs(d).into_iter().filter(|(c, _)| (1000..2000).contains(c)).collect()
}

// ---- real zsh ---------------------------------------------------------------------------

pub const ZSH: &str = "/usr/bin/zsh";

pub fn have_zsh() -> bool {
    std::path::Path::new(ZSH).exists()
}

/// `zsh -fn` accepts the script.
pub fn zsh_n_ok(script: &str) -> bool {
    let mut child = Command::new(ZSH)
        .args(["-f", "-n"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    child.wait().unwrap().success()
}

/// Run `body` under `zsh -f -c` and return stdout.
pub fn zsh_run(body: &str) -> String {
    let o = Command::new(ZSH).args(["-f", "-c", body]).output().unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Ground truth for the option tests. Runs `script` in real zsh with every `echo $1`
/// replaced by a probe that reports its own line number and how many words `$1` became
/// (`$1` is "a b" at top level, so more than one word means zsh split it). Returns the
/// lines at which splitting happened. Line numbers are preserved (the helper is
/// prepended on line 1).
pub fn zsh_split_lines(script: &str) -> Vec<u32> {
    let probe = "n() { print -r -- \"$1:$(( $# - 1 ))\" }; set -- 'a b'; ";
    // (`$LINENO` is relative to the enclosing function in zsh, so bake the file line in.)
    let body: String = script
        .split_inclusive('\n')
        .enumerate()
        .map(|(i, l)| l.replace("echo $1", &format!("n {} $1", i + 1)))
        .collect();
    let full = format!("{probe}{body}");
    let dir = std::env::temp_dir().join(format!(
        "shelllint-zsh-oracle-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("oracle.zsh");
    std::fs::write(&path, full).unwrap();
    let o = Command::new(ZSH).args(["-f"]).arg(&path).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let mut v: Vec<u32> = out
        .lines()
        .filter_map(|l| {
            let (line, n) = l.split_once(':')?;
            let n: u32 = n.trim().parse().ok()?;
            if n > 1 { line.trim().parse().ok() } else { None }
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// SC`code` is reported on exactly `expect` lines under `--shell=zsh`.
pub fn code_lint(code: u32, script: &str, expect: &[u32]) {
    let got = zsh(script);
    assert_eq!(lines(&got, code), expect, "SC{code} lines under zsh (all findings {:?}) for:\n{script}", pairs(&got));
}
