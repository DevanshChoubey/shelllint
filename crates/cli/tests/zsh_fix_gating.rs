// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression tests (no ShellCheck counterpart). The
// zsh anonymous-function name must not change how other shells are checked.
#![allow(dead_code)]

#[path = "zsh_common.rs"]
mod zsh_common;
use std::io::Write;
use std::process::{Command, Stdio};
use zsh_common::*;

/// Codes for `script` read from stdin under `--shell=<shell>`, in report order.
fn codes_under(shell: &str, script: &str) -> Vec<u32> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1", &format!("--shell={shell}"), "-"])
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    v["comments"].as_array().unwrap().iter().map(|c| c["code"].as_u64().unwrap() as u32).collect()
}

/// A user function that happens to share the zsh anonymous-function name,
/// defined and then called bare. ShellCheck pairs the call with the definition.
const NAMED_LIKE_ZSH_ANON: &str = "zsh-anonymous-function() { echo \"$1\"; }\nzsh-anonymous-function\n";

#[test]
fn bash_user_function_named_like_the_zsh_anonymous_one_is_checked() {
    // oracle (shellcheck-hs -s bash): SC2120 on the definition, SC2119 on the call
    assert_eq!(codes_under("bash", NAMED_LIKE_ZSH_ANON), vec![2120, 2119]);
}

#[test]
fn ksh_user_function_named_like_the_zsh_anonymous_one_is_checked() {
    // oracle (shellcheck-hs -s ksh)
    assert_eq!(codes_under("ksh", NAMED_LIKE_ZSH_ANON), vec![2120, 2119]);
}

#[test]
fn sh_user_function_named_like_the_zsh_anonymous_one_is_checked() {
    // oracle (shellcheck-hs -s sh): also SC3033, a POSIX-sh portability note
    assert_eq!(codes_under("sh", NAMED_LIKE_ZSH_ANON), vec![2120, 3033, 2119]);
}

#[test]
fn dash_user_function_named_like_the_zsh_anonymous_one_is_checked() {
    // oracle (shellcheck-hs -s dash)
    assert_eq!(codes_under("dash", NAMED_LIKE_ZSH_ANON), vec![3033, 2120, 2119]);
}

#[test]
fn bash_plain_function_pairing_is_unchanged() {
    // the same pairing under a name that was never special
    assert_eq!(bash_codes("other() { echo \"$1\"; }\nother\n"), vec![2120, 2119]);
}
