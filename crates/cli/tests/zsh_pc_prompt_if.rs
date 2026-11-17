// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (lane P-C): SC1083 on prompt escapes and if without then (C24, C28).
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

const C24_PROMPT_BRACES_POSITIVES: &[(&str, &str)] = &[
    (r#"prompt_escape_pair"#, r#"PS1=%{x%}
"#),
    (r#"color_escape"#, r#"PS1=%F{blue}x%f
"#),
    (r#"nested_prompt_escapes"#, r#"PROMPT=%{%F{blue}%}x
"#),
    (r#"revision_suffix"#, r#"git log @{u}..
"#),
    (r#"revision_suffix_negative_index"#, r#"git rev-parse HEAD@{-1}
"#),
    (r#"revision_in_command_subst"#, r#"x=$(git rev-parse @{upstream})
"#),
    (r#"background_color_escape"#, r#"PS1=%K{red}x%k
"#),
    (r#"strftime_escape"#, r#"PS1=%D{"%H:%M"}
"#),
    (r#"escape_with_expansion"#, r#"RPROMPT=%{$fg[red]%}x%{$reset_color%}
"#),
];

#[test]
fn c24_prompt_braces_positives_parse_under_zsh() {
    for (name, script) in C24_PROMPT_BRACES_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c24_prompt_braces_positives_do_not_crash_any_check() {
    for (name, script) in C24_PROMPT_BRACES_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C24_PROMPT_BRACES_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"prompt_escape_pair"#, &[1083, 1083]),
    (r#"color_escape"#, &[1083, 1083]),
    (r#"nested_prompt_escapes"#, &[2034, 1083, 1083, 1083, 1083]),
    (r#"revision_suffix"#, &[1083, 1083]),
    (r#"revision_suffix_negative_index"#, &[1083, 1083]),
    (r#"revision_in_command_subst"#, &[2034, 1083, 1083]),
    (r#"background_color_escape"#, &[1083, 1083]),
    (r#"strftime_escape"#, &[1083, 1083]),
    (r#"escape_with_expansion"#, &[2034, 2125, 1083, 1087, 2154, 1083, 1083, 2154, 1083]),
];

#[test]
fn c24_prompt_braces_bash_output_is_unchanged() {
    let table = C24_PROMPT_BRACES_POSITIVES;
    assert_eq!(names(table), C24_PROMPT_BRACES_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C24_PROMPT_BRACES_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

#[test]
fn c24_prompt_braces_literal_close_brace_still_warns() {
    let script = r#"echo a}b
"#;
    assert!(zsh_codes(script).contains(&1083), "zsh keeps SC1083 for literal_close_brace_still_warns");
    assert_eq!(zsh_codes(script), bash_codes(script));
    assert_eq!(bash_codes(script), [1083]);
}

#[test]
fn c24_prompt_braces_literal_open_brace_still_warns() {
    let script = r#"echo {a
"#;
    assert!(zsh_codes(script).contains(&1083), "zsh keeps SC1083 for literal_open_brace_still_warns");
    assert_eq!(zsh_codes(script), bash_codes(script));
    assert_eq!(bash_codes(script), [1083]);
}

const C28_IF_WITHOUT_THEN_POSITIVES: &[(&str, &str)] = &[
    (r#"in_else_branch"#, r#"if true; then echo a
else
  if ! [[ -z $x ]] && printf '%s' "$x"
  local s=1
  printf '%s' "$s"
fi
"#),
    (r#"in_brace_group"#, r#"{
if true
local s=1
}
"#),
    (r#"in_function"#, r#"g() {
  if [[ -n $1 ]] && print x
  local s=1
}
"#),
    (r#"in_subshell"#, r#"(
  if true
  print a
)
"#),
    (r#"closed_by_fi_of_outer_if"#, r#"if true; then echo a
else
if true
local s=1
fi
"#),
    (r#"condition_only"#, r#"{ if true; }
"#),
];

#[test]
fn c28_if_without_then_positives_parse_under_zsh() {
    for (name, script) in C28_IF_WITHOUT_THEN_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c28_if_without_then_positives_do_not_crash_any_check() {
    for (name, script) in C28_IF_WITHOUT_THEN_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C28_IF_WITHOUT_THEN_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"in_else_branch"#, &[1009, 1049, 1073, 1050, 1072]),
    (r#"in_brace_group"#, &[1009, 1073, 1050, 1072]),
    (r#"in_function"#, &[1009, 1073, 1050, 1072]),
    (r#"in_subshell"#, &[1009, 1073, 1050, 1072]),
    (r#"closed_by_fi_of_outer_if"#, &[1009, 1049, 1073, 1050, 1072]),
    (r#"condition_only"#, &[1009, 1073, 1050, 1072]),
];

#[test]
fn c28_if_without_then_bash_output_is_unchanged() {
    let table = C28_IF_WITHOUT_THEN_POSITIVES;
    assert_eq!(names(table), C28_IF_WITHOUT_THEN_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C28_IF_WITHOUT_THEN_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C28_IF_WITHOUT_THEN_NEAR_MISSES: &[(&str, &str)] = &[
    (r#"top_level_without_then"#, r#"if true
local s=1
"#),
    (r#"top_level_if_fi"#, r#"if true
fi
"#),
    (r#"extra_fi_after_short_if"#, r#"if true; then
else if [[ -z $x ]]
  printf hi
  fi
fi
"#),
];

#[test]
fn c28_if_without_then_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C28_IF_WITHOUT_THEN_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

