// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (lane P-C): forced append redirections and numeric fd variables (C19, C20).
//
// Every positive snippet below was accepted by `zsh -n` (zsh 5.9) and every
// near-miss was rejected by it when this file was generated. The bash
// guards pin the output of a build from before the zsh parser rules.
#![allow(clippy::type_complexity)]

#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

/// Runs every check (`--enable=all`) over `script` under zsh: the new AST
/// shapes (an `if` with an empty body, a loop with an extra name, `name()` and
/// `[##16]` arithmetic nodes) must not crash any check.
fn assert_no_crash_with_all_checks(name: &str, script: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "--shell=zsh", "--enable=all", "-f", "json1", "-"])
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
        "`{name}` crashed with --enable=all (exit {:?}): {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&o.stdout).expect("json1 output");
}

fn names(table: &[(&str, &str)]) -> Vec<String> {
    table.iter().map(|(n, _)| n.to_string()).collect()
}

/// A parse error is reported under zsh: one of the 12 hard codes, or the
/// "parsing stopped here" / unterminated-`if` family (1089, 1046, 1047, 1050).
fn assert_hard_error(name: &str, script: &str) {
    let got = zsh_codes(script);
    assert!(
        got.iter().any(|c| HARD_PARSE_CODES.contains(c) || [1089, 1046, 1047, 1050].contains(c)),
        "near-miss `{name}`: zsh rejects it, shelllint reported {got:?} for:\n{script}"
    );
}

const C19_CLOBBER_APPEND_POSITIVES: &[(&str, &str)] = &[
    (r#"force_append"#, r#"echo a >>| f
"#),
    (r#"force_append_bang"#, r#"echo a >>! f
"#),
    (r#"force_truncate_bang"#, r#"echo a >! f
"#),
    (r#"fd_prefixed"#, r#"cmd 2>>| log
"#),
    (r#"quoted_target_in_subshell"#, r#"( umask 077; : >>| "$datafile" ) 2> /dev/null
"#),
    (r#"on_brace_group"#, r#"{ echo a; } >>| f
"#),
    (r#"in_and_or_list"#, r#"echo a >>| f && echo b >>! f || echo c
"#),
];

#[test]
fn c19_clobber_append_positives_parse_under_zsh() {
    for (name, script) in C19_CLOBBER_APPEND_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c19_clobber_append_positives_do_not_crash_any_check() {
    for (name, script) in C19_CLOBBER_APPEND_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C19_CLOBBER_APPEND_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"force_append"#, &[1009, 1073, 1072]),
    (r#"force_append_bang"#, &[]),
    (r#"force_truncate_bang"#, &[]),
    (r#"fd_prefixed"#, &[1009, 1073, 1072]),
    (r#"quoted_target_in_subshell"#, &[1009, 1073, 1072]),
    (r#"on_brace_group"#, &[1073, 1072]),
    (r#"in_and_or_list"#, &[1009, 1073, 1072]),
];

#[test]
fn c19_clobber_append_bash_output_is_unchanged() {
    let table = C19_CLOBBER_APPEND_POSITIVES;
    assert_eq!(names(table), C19_CLOBBER_APPEND_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C19_CLOBBER_APPEND_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C19_CLOBBER_APPEND_NEAR_MISSES: &[(&str, &str)] = &[
    (r#"missing_target"#, r#"echo a >>|
"#),
];

#[test]
fn c19_clobber_append_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C19_CLOBBER_APPEND_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

const C20_FD_DIGIT_VAR_POSITIVES: &[(&str, &str)] = &[
    (r#"close_numeric_fd"#, r#"exec {1}<&-
"#),
    (r#"builtin_exec_in_function"#, r#"g() {
  builtin exec {1}<&-
}
"#),
    (r#"open_numeric_fd"#, r#"exec {10}>file
"#),
    (r#"close_output_fd"#, r#"exec {1}>&-
"#),
    (r#"after_command"#, r#"echo hi {1}>file
"#),
    (r#"named_fd_still_works"#, r#"exec {fd}<&-
"#),
    (r#"in_condition_branch"#, r#"if true; then exec {1}<&-; fi
"#),
];

#[test]
fn c20_fd_digit_var_positives_parse_under_zsh() {
    for (name, script) in C20_FD_DIGIT_VAR_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c20_fd_digit_var_positives_do_not_crash_any_check() {
    for (name, script) in C20_FD_DIGIT_VAR_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C20_FD_DIGIT_VAR_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"close_numeric_fd"#, &[1083, 1083]),
    (r#"builtin_exec_in_function"#, &[1083, 1083]),
    (r#"open_numeric_fd"#, &[1083, 1083]),
    (r#"close_output_fd"#, &[1083, 1083]),
    (r#"after_command"#, &[1083, 1083]),
    (r#"named_fd_still_works"#, &[2154]),
    (r#"in_condition_branch"#, &[1083, 1083]),
];

#[test]
fn c20_fd_digit_var_bash_output_is_unchanged() {
    let table = C20_FD_DIGIT_VAR_POSITIVES;
    assert_eq!(names(table), C20_FD_DIGIT_VAR_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C20_FD_DIGIT_VAR_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C20_FD_DIGIT_VAR_NEAR_MISSES: &[(&str, &str)] = &[
    (r#"missing_dup_target"#, r#"exec {1}<&
"#),
];

#[test]
fn c20_fd_digit_var_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C20_FD_DIGIT_VAR_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

