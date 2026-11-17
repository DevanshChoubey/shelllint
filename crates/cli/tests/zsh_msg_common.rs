// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only test helpers for the zsh message lane (no ShellCheck counterpart).
//
// Runs the built `shelllint` binary and a real zsh, so a test can state a zsh
// claim and check it against zsh in the same place. The tests that use it need
// zsh installed (`/usr/bin/zsh` or `/bin/zsh`); CI installs it on Linux.
#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};

/// The zsh binary to check claims against.
pub fn zsh_bin() -> &'static str {
    ["/usr/bin/zsh", "/bin/zsh"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .expect("these tests check their zsh claims against a real zsh: install zsh")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub code: u32,
    pub message: String,
    pub has_fix: bool,
}

pub fn lint(args: &[&str], script: &str) -> Vec<Finding> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1"])
        .args(args)
        .arg("-")
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(
        matches!(o.status.code(), Some(0) | Some(1)),
        "exit {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    v["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| Finding {
            code: c["code"].as_u64().unwrap() as u32,
            message: c["message"].as_str().unwrap().to_string(),
            has_fix: !c["fix"].is_null(),
        })
        .collect()
}

pub fn zsh(script: &str) -> Vec<Finding> {
    lint(&["--shell=zsh"], script)
}
pub fn bash(script: &str) -> Vec<Finding> {
    lint(&["--shell=bash"], script)
}
pub fn ksh(script: &str) -> Vec<Finding> {
    lint(&["--shell=ksh"], script)
}

pub fn codes(f: &[Finding]) -> Vec<u32> {
    f.iter().map(|x| x.code).collect()
}

/// The message of the only finding with `code`.
pub fn message_of(f: &[Finding], code: u32) -> String {
    let m: Vec<&Finding> = f.iter().filter(|x| x.code == code).collect();
    assert_eq!(m.len(), 1, "expected exactly one SC{code} in {f:?}");
    m[0].message.clone()
}

/// `zsh -n` accepts `script` (parse only).
pub fn zsh_parses(script: &str) {
    let o = Command::new(zsh_bin()).args(["-fn", "-c", script]).output().unwrap();
    assert!(o.status.success(), "zsh -n rejected {script:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// stdout of `zsh -fc script` (stderr folded in), trimmed.
pub fn zsh_run(script: &str) -> String {
    let o = Command::new(zsh_bin()).args(["-fc", script]).output().unwrap();
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    s.trim().to_string()
}

/// Codes this lane owns; `lane` drops everything else (SC2034, SC2154, ... depend on
/// other lanes' work and would make these tests brittle).
pub const LANE_CODES: &[u32] = &[
    2016, 2051, 2082, 2118, 2180, 2206, 2207, 2270, 2271, 2272, 2273, 2274, 2275, 2276, 2277, 2278, 2279, 2280, 2281,
    2282, 2296, 2297, 2298, 2299, 2300, 2301, 2302, 2303, 2338,
];

pub fn lane(f: &[Finding]) -> Vec<u32> {
    f.iter().map(|x| x.code).filter(|c| LANE_CODES.contains(c)).collect()
}
