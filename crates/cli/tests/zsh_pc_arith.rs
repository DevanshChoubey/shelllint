// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (lane P-C): arithmetic base prefix and math function calls (C18, C23).
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

const C18_ARITH_BASE_POSITIVES: &[(&str, &str)] = &[
    (
        r#"double_hash_base_16"#,
        r#"echo $(( [##16] 255 ))
"#,
    ),
    (
        r#"single_hash_base_16"#,
        r#"echo $(( [#16] 255 ))
"#,
    ),
    (
        r#"base_in_arith_command_assignment"#,
        r#"(( x = [#16] 255 ))
"#,
    ),
    (
        r#"base_36_with_expression"#,
        r#"x=$(( [##36] 99999 + 1 ))
"#,
    ),
    (
        r#"base_after_operator"#,
        r#"echo $(( 2 * [#16] 5 ))
"#,
    ),
    (
        r#"base_with_char_code_operand"#,
        r#"ord=$(( [##16] #byte ))
"#,
    ),
    (
        r#"base_without_space"#,
        r#"echo $(( [##16]255+1 ))
"#,
    ),
    (
        r#"in_function_in_pipeline"#,
        r#"g() { echo $(( [##2] $1 )) | tr 0 o; }
"#,
    ),
    (
        r#"base_from_parameter"#,
        r#"echo $(( [#$b] 5 ))
"#,
    ),
];

#[test]
fn c18_arith_base_positives_parse_under_zsh() {
    for (name, script) in C18_ARITH_BASE_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c18_arith_base_positives_do_not_crash_any_check() {
    for (name, script) in C18_ARITH_BASE_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C18_ARITH_BASE_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"double_hash_base_16"#, &[1073, 1009, 1072]),
    (r#"single_hash_base_16"#, &[1073, 1009, 1072]),
    (r#"base_in_arith_command_assignment"#, &[1105, 2205, 2283]),
    (r#"base_36_with_expression"#, &[1073, 1009, 1072]),
    (r#"base_after_operator"#, &[1102, 2046, 2005, 2035]),
    (r#"base_with_char_code_operand"#, &[1073, 1009, 1072]),
    (r#"base_without_space"#, &[1073, 1009, 1072]),
    (r#"in_function_in_pipeline"#, &[1073, 1009, 1072]),
    (r#"base_from_parameter"#, &[1073, 1009, 1072]),
];

#[test]
fn c18_arith_base_bash_output_is_unchanged() {
    let table = C18_ARITH_BASE_POSITIVES;
    assert_eq!(names(table), C18_ARITH_BASE_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C18_ARITH_BASE_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C18_ARITH_BASE_NEAR_MISSES: &[(&str, &str)] = &[(
    r#"unterminated_arith"#,
    r#"echo $(( [##16] 255 )
"#,
)];

#[test]
fn c18_arith_base_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C18_ARITH_BASE_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}

const C23_MATH_FUNCTION_POSITIVES: &[(&str, &str)] = &[
    (
        r#"empty_argument_list"#,
        r#"(( f() == 1 ))
"#,
    ),
    (
        r#"mathfunc_no_args"#,
        r#"(( rand48() < 2 ))
"#,
    ),
    (
        r#"with_arguments"#,
        r#"(( f(1, 2) > 0 )) && echo y
"#,
    ),
    (
        r#"in_expansion_assignment"#,
        r#"x=$(( abs(-3) + 1 ))
"#,
    ),
    (
        r#"in_if_condition"#,
        r#"if (( sqrt(16) == 4 )); then echo y; fi
"#,
    ),
    (
        r#"nested_calls"#,
        r#"echo $(( f(g(1)) ))
"#,
    ),
    (
        r#"with_space_before_paren_in_group"#,
        r#"(( (f()) + 1 ))
"#,
    ),
    (
        r#"combined_with_base_prefix"#,
        r#"echo $(( [##16] abs(-255) ))
"#,
    ),
];

#[test]
fn c23_math_function_positives_parse_under_zsh() {
    for (name, script) in C23_MATH_FUNCTION_POSITIVES {
        let got = zsh_codes(script);
        let hard: Vec<u32> = got.iter().copied().filter(|c| HARD_PARSE_CODES.contains(c)).collect();
        assert!(hard.is_empty(), "positive `{name}`: zsh parse errors {hard:?} (all codes {got:?}) for:\n{script}");
    }
}

#[test]
fn c23_math_function_positives_do_not_crash_any_check() {
    for (name, script) in C23_MATH_FUNCTION_POSITIVES {
        assert_no_crash_with_all_checks(name, script);
    }
}

/// Dialect guard: the same snippets under bash give what they gave before the zsh parser.
const C23_MATH_FUNCTION_BASH_TODAY: &[(&str, &[u32])] = &[
    (r#"empty_argument_list"#, &[1073, 1009, 1072]),
    (r#"mathfunc_no_args"#, &[1073, 1009, 1072]),
    (r#"with_arguments"#, &[1073, 1009, 1072]),
    (r#"in_expansion_assignment"#, &[1073, 1009, 1072]),
    (r#"in_if_condition"#, &[1073, 1009, 1072]),
    (r#"nested_calls"#, &[1073, 1009, 1072]),
    (r#"with_space_before_paren_in_group"#, &[1073, 1009, 1072]),
    (r#"combined_with_base_prefix"#, &[1073, 1009, 1072]),
];

#[test]
fn c23_math_function_bash_output_is_unchanged() {
    let table = C23_MATH_FUNCTION_POSITIVES;
    assert_eq!(names(table), C23_MATH_FUNCTION_BASH_TODAY.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for ((name, script), (_, today)) in table.iter().zip(C23_MATH_FUNCTION_BASH_TODAY) {
        assert_eq!(bash_codes(script), *today, "bash output changed for `{name}`:\n{script}");
    }
}

const C23_MATH_FUNCTION_NEAR_MISSES: &[(&str, &str)] = &[(
    r#"unbalanced_call"#,
    r#"(( f(1 ))
"#,
)];

#[test]
fn c23_math_function_near_misses_zsh_rejects_are_not_accepted() {
    for (name, script) in C23_MATH_FUNCTION_NEAR_MISSES {
        assert_hard_error(name, script);
    }
}
