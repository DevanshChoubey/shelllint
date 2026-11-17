// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only test helpers for the zsh work (no ShellCheck counterpart).
//
// Shared helpers for the zsh test files. A test file pulls them in with
//
//     #[path = "zsh_common.rs"]
//     mod zsh_common;
//     use zsh_common::*;
//
// They run the built `shelllint` binary on a script from stdin with `--norc`
// and read the codes out of `-f json1`, in report order. This file is also a
// test target of its own, whose tests exercise each helper once.
#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};

/// The parse failures zsh scripts must not hit: the 12 hard parse codes.
pub const HARD_PARSE_CODES: [u32; 12] = [1072, 1073, 1036, 1058, 1070, 1141, 1054, 1056, 1055, 1009, 1027, 1088];

fn codes_for(args: &[&str], script: &str) -> Vec<u32> {
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
        "shelllint {args:?} exited {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    v["comments"].as_array().unwrap().iter().map(|c| c["code"].as_u64().unwrap() as u32).collect()
}

/// Codes reported for `script` under `--shell=zsh`, in report order.
pub fn zsh_codes(script: &str) -> Vec<u32> {
    codes_for(&["--shell=zsh"], script)
}

/// Like [`zsh_codes`], with one `setopt <name>` line per entry of `setopts`
/// put in front of the script (so every reported line number is shifted by
/// `setopts.len()`). Spell an option however `setopt` accepts it, `noX` included.
pub fn zsh_codes_opts(script: &str, setopts: &[&str]) -> Vec<u32> {
    let mut full = String::new();
    for o in setopts {
        full.push_str(&format!("setopt {o}\n"));
    }
    full.push_str(script);
    zsh_codes(&full)
}

/// Codes reported for `script` under `--shell=bash`, in report order.
pub fn bash_codes(script: &str) -> Vec<u32> {
    codes_for(&["--shell=bash"], script)
}

/// `script` parses under zsh: none of the 12 hard parse codes
/// ([`HARD_PARSE_CODES`]) is reported. Other findings are fine.
pub fn assert_parses(script: &str) {
    let got = zsh_codes(script);
    let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
    assert!(hard.is_empty(), "zsh parse errors {hard:?} (all codes {got:?}) for script:\n{script}");
}

/// Dialect guard: under `--shell=bash` the snippet gives exactly `today`, the
/// codes the tree produced before zsh support (take them from a build of main).
/// A zsh rule that leaks into bash changes this list. Compare with the full
/// json1 output in the freeze test when the codes alone are too coarse.
pub fn assert_same_as_bash_today(script: &str, today: &[u32]) {
    assert_eq!(bash_codes(script), today, "bash output changed for script:\n{script}");
}

#[test]
fn zsh_is_accepted_and_clean_scripts_stay_clean() {
    assert_eq!(zsh_codes("echo \"hi\"\n"), Vec::<u32>::new());
}

#[test]
fn setopt_lines_are_prepended() {
    assert_eq!(zsh_codes_opts("echo \"hi\"\n", &["extendedglob", "nonomatch"]), Vec::<u32>::new());
    assert_eq!(zsh_codes_opts("if true; then echo hi\n", &["localoptions"])[0], 1046);
}

#[test]
fn assert_parses_accepts_plain_syntax() {
    assert_parses("f() { echo \"$1\"; }\nf a\n");
    assert_parses("[[ $x == a* ]]\n");
}

#[test]
#[should_panic(expected = "zsh parse errors")]
fn assert_parses_rejects_a_syntax_error_zsh_also_rejects() {
    // `zsh -n` fails on this too ("parse error near `\n'"): an unterminated `if`.
    assert_parses("if true; then echo hi\n");
}

#[test]
fn bash_guard_pins_bash_output() {
    assert_same_as_bash_today("[[ $x == a* ]]\n", &[2154]);
    assert_same_as_bash_today("for f in a b; do echo $f; done\n", &[]);
}

#[test]
#[should_panic(expected = "bash output changed")]
fn bash_guard_fails_on_a_difference() {
    assert_same_as_bash_today("[[ $x == a* ]]\n", &[]);
}
