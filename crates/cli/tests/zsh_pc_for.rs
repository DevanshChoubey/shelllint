// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (lane P-C): for x (list), SHORT_LOOPS bodies and for k v in (C08, C09).
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

const C08_FOR_SHORT_POSITIVES: &[(&str, &str)] = &[
    (r#"short_body_same_line"#, r#"for f (a b) echo $f
"#),
    (r#"short_body_after_semicolon"#, r#"for f (a b); echo $f
"#),
    (r#"multi_line_word_list"#, r#"for f (a
b) echo $f
"#),
    (r#"body_on_next_line"#, r#"for f (a b)
  echo $f
"#),
    (r#"brace_body"#, r#"for f (a b) { echo $f; }
"#),
    (r#"brace_body_after_semicolon"#, r#"for f (a b); { echo $f; }
"#),
    (r#"brace_body_on_next_line"#, r#"for f (a b)
{ echo $f; }
"#),
    (r#"for_in_brace_body"#, r#"for f in a b; { echo $f; }
"#),
    (r#"for_in_short_body_after_semicolon"#, r#"for f in a b; echo $f
"#),
    (r#"for_in_short_body_next_line"#, r#"for f in a b
echo $f
"#),
    (r#"do_done_after_paren_list"#, r#"for f (a b); do echo $f; done
"#),
    (r#"do_done_on_next_line"#, r#"for f (a b)
do echo $f; done
"#),
    (r#"in_function_with_pipeline"#, r#"g() { for x ($@) echo $x | sort; }
"#),
    (r#"nested_short_loops"#, r#"for f (a b) for g (c d) echo $f$g
"#),
    (r#"short_body_with_and_list"#, r#"for f (a b) echo $f && echo next
"#),
    (r#"arithmetic_for_short_body"#, r#"for ((i=0;i<3;i++)) echo $i
"#),
    (r#"arithmetic_for_short_body_semicolon"#, r#"for ((i=0;i<3;i++)); echo $i
"#),
    (r#"followed_by_command"#, r#"for f (a b) echo $f
echo done
"#),
    (r#"inside_if"#, r#"if true; then for f (a b) echo $f; fi
"#),
    (r#"quoted_and_expanded_words"#, r#"for f ("a b" $x ${y:-z}) echo $f
"#),
];

#[test]
fn c08_for_short_positives_parse_under_zsh() {
    for (name, script) in C08_FOR_SHORT_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c08_for_short_positives_do_not_crash_any_check() {
    for (name, script) in C08_FOR_SHORT_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C08_FOR_SHORT_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"short_body_same_line"#, &[1073, 1058, 1072]),
    (r#"short_body_after_semicolon"#, &[1073, 1058, 1072]),
    (r#"multi_line_word_list"#, &[1073, 1058, 1072]),
    (r#"body_on_next_line"#, &[1073, 1058, 1072]),
    (r#"brace_body"#, &[1073, 1058, 1072]),
    (r#"brace_body_after_semicolon"#, &[1073, 1058, 1072]),
    (r#"brace_body_on_next_line"#, &[1073, 1058, 1072]),
    (r#"for_in_brace_body"#, &[]),
    (r#"for_in_short_body_after_semicolon"#, &[1073, 1058, 1072]),
    (r#"for_in_short_body_next_line"#, &[1073, 1058, 1072]),
    (r#"do_done_after_paren_list"#, &[1073, 1058, 1072]),
    (r#"do_done_on_next_line"#, &[1073, 1058, 1072]),
    (r#"in_function_with_pipeline"#, &[1009, 1073, 1058, 1072]),
    (r#"nested_short_loops"#, &[1073, 1058, 1072]),
    (r#"short_body_with_and_list"#, &[1073, 1058, 1072]),
    (r#"arithmetic_for_short_body"#, &[1009, 1073, 1058, 1072]),
    (r#"arithmetic_for_short_body_semicolon"#, &[1009, 1073, 1058, 1072]),
    (r#"followed_by_command"#, &[1073, 1058, 1072]),
    (r#"inside_if"#, &[1009, 1073, 1058, 1072]),
    (r#"quoted_and_expanded_words"#, &[1073, 1058, 1072]),
];

#[test]
fn c08_for_short_bash_output_is_unchanged() {
    let table = C08_FOR_SHORT_POSITIVES;
    assert_eq!(names(table), C08_FOR_SHORT_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C08_FOR_SHORT_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C08_FOR_SHORT_NEAR_MISSES: &[(&str, &str)] = &[
    (r#"unterminated_word_list"#, r#"for f (a b
"#),
    (r#"stray_done_after_short_body"#, r#"for f (a b) echo $f; done
"#),
    (r#"do_without_done"#, r#"for f (a b) do echo $f
"#),
];

#[test]
fn c08_for_short_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C08_FOR_SHORT_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

const C09_FOR_MULTI_VAR_POSITIVES: &[(&str, &str)] = &[
    (r#"two_names_in_list"#, r#"for k v in a 1 b 2; do echo "$k $v"; done
"#),
    (r#"three_names"#, r#"for k v w in a 1 2 b 3 4; do echo "$k$v$w"; done
"#),
    (r#"names_with_paren_list"#, r#"for k v (a 1 b 2) echo "$k=$v"
"#),
    (r#"names_with_brace_body"#, r#"for k v in a 1 b 2; { echo "$k=$v"; }
"#),
    (r#"over_assoc_array"#, r#"g() { local -A m; m=(a 1); for k v in ${(kv)m}; do print -r -- "$k $v"; done; }
"#),
    (r#"positional_params"#, r#"for k v; do echo "$k$v"; done
"#),
    (r#"multi_line_loop"#, r#"for k v in a 1 b 2
do
  echo "$k"
  echo "$v"
done
"#),
    (r#"digit_names_loop_over_positionals"#, r#"set -- a b c d e f
for 1 2 3; do print "$1$2$3"; done
"#),
    (r#"digit_name_with_in_list"#, r#"for 1 in a b; do print "$1"; done
"#),
];

#[test]
fn c09_for_multi_var_positives_parse_under_zsh() {
    for (name, script) in C09_FOR_MULTI_VAR_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c09_for_multi_var_positives_do_not_crash_any_check() {
    for (name, script) in C09_FOR_MULTI_VAR_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C09_FOR_MULTI_VAR_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"two_names_in_list"#, &[1073, 1058, 1072]),
    (r#"three_names"#, &[1073, 1058, 1072]),
    (r#"names_with_paren_list"#, &[1073, 1058, 1072]),
    (r#"names_with_brace_body"#, &[1073, 1058, 1072]),
    (r#"over_assoc_array"#, &[1009, 1073, 1058, 1072]),
    (r#"positional_params"#, &[1073, 1058, 1072]),
    (r#"multi_line_loop"#, &[1073, 1058, 1072]),
    (r#"digit_names_loop_over_positionals"#, &[1073, 1072]),
    (r#"digit_name_with_in_list"#, &[1073, 1072]),
];

#[test]
fn c09_for_multi_var_bash_output_is_unchanged() {
    let table = C09_FOR_MULTI_VAR_POSITIVES;
    assert_eq!(names(table), C09_FOR_MULTI_VAR_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C09_FOR_MULTI_VAR_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C09_FOR_MULTI_VAR_NEAR_MISSES: &[(&str, &str)] = &[
    (r#"missing_done"#, r#"for k v in a 1 b 2; do echo hi
"#),
    (r#"missing_closing_paren"#, r#"for k v (a 1 b 2
"#),
    (r#"digit_then_letter_is_not_a_name"#, r#"for 1x in a; do :; done
"#),
];

#[test]
fn c09_for_multi_var_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C09_FOR_MULTI_VAR_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

