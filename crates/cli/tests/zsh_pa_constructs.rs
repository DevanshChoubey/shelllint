// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (function and group
// syntax: constructs C01 C02 C03 C10 C11 C12 C13 C14 C15 C16 C22 C25). Layer 2
// of the zsh test strategy, per construct:
//   - positives: every snippet is accepted by `zsh -n` (zsh 5.9, checked when
//     the test was written) and parses under `--shell=zsh` with exactly the
//     recorded codes;
//   - dialect guard: the same snippets under `--shell=bash` give exactly what
//     the tree gave before zsh support;
//   - near-misses: snippets `zsh -n` rejects, which `--shell=zsh` must also
//     report as a parse error.
// Scripts select zsh with `--shell=zsh`, never with a shebang alone (see
// `zsh_shebang_body_pending`).

#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

/// The hard parse codes plus SC1089 ("parsing stopped here"), which zsh
/// errors such as a stray `}` come out as.
const PARSE_ERRORS: [u32; 13] = [1072, 1073, 1036, 1058, 1070, 1141, 1054, 1056, 1055, 1009, 1027, 1088, 1089];

/// (script, codes under `--shell=zsh`, codes under `--shell=bash` before zsh support)
type Case = (&'static str, &'static [u32], &'static [u32]);

fn check_positives(cases: &[Case]) {
    for (script, zsh, _) in cases {
        assert_parses(script);
        assert_eq!(zsh_codes(script), *zsh, "zsh codes for script:\n{script}");
    }
}

fn check_bash_unchanged(cases: &[Case]) {
    for (script, _, bash) in cases {
        assert_same_as_bash_today(script, bash);
    }
}

fn check_near_misses(cases: &[Case]) {
    for (script, _, _) in cases {
        let got = zsh_codes(script);
        assert!(
            got.iter().any(|c| PARSE_ERRORS.contains(c)),
            "zsh -n rejects this script, but --shell=zsh reported only {got:?}:\n{script}"
        );
    }
}

// ---- C01: anonymous function --------------------------------------
const C01_POSITIVES: &[Case] = &[
    ("() { echo hi }\n", &[], &[1073, 1072]),
    ("function { echo hi }\n", &[], &[1083, 1083]),
    ("f() { () { echo nested }; }\nf\n", &[], &[1009, 1073, 1072]),
    ("() { echo \"$1\" } a b\n", &[], &[1073, 1072]),
    ("() {\n  local x=1\n  echo \"$x\"\n}\n", &[], &[1073, 1072]),
    ("() ( echo sub )\n", &[], &[1073, 1072]),
    ("function () { echo hi }\n", &[], &[1036, 1088]),
    ("() { echo hi } | cat\n", &[], &[1073, 1072]),
    ("if true; then () { echo hi }; fi\n", &[], &[1009, 1073, 1072]),
    ("() {\n  echo hi\n} > /dev/null\n", &[], &[1073, 1072]),
];
const C01_NEAR_MISSES: &[Case] = &[
    ("(  ) { :; }\n", &[1073, 1072], &[1073, 1072]),
    ("x=() { :; }\n", &[1083, 2288, 1089], &[1083, 2288, 1089]),
    ("() {\n", &[1073, 1072, 1009], &[1073, 1072]),
];

#[test]
fn c01_anonymous_function_parses_in_zsh() {
    check_positives(C01_POSITIVES);
}

#[test]
fn c01_anonymous_function_bash_output_is_unchanged() {
    check_bash_unchanged(C01_POSITIVES);
    check_bash_unchanged(C01_NEAR_MISSES);
}

#[test]
fn c01_anonymous_function_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C01_NEAR_MISSES);
}

// ---- C02: disown ampersand ----------------------------------------
const C02_POSITIVES: &[Case] = &[
    ("sleep 1 &|\n", &[], &[1070]),
    ("sleep 1 &!\n", &[], &[1035, 1072]),
    ("f() { sleep 1 &| }\nf\n", &[], &[1009, 1073, 1056, 1072]),
    ("sleep 1 &|\necho next\n", &[], &[1070]),
    ("sleep 1 &| sleep 2 &! sleep 3\n", &[], &[1070]),
    ("{ sleep 1 &| }\n", &[], &[1073, 1056, 1072]),
    ("sleep 1 && sleep 2 &|\n", &[], &[1070]),
    ("sleep 1 | cat &!\n", &[], &[1035, 1072]),
    ("if true; then sleep 1 &|\nfi\n", &[], &[1046, 1073, 1047, 1072]),
];
const C02_NEAR_MISSES: &[Case] = &[
    ("sleep 1 &|| sleep 2\n", &[1070], &[1070]),
    ("sleep 1 &! &!\n", &[1070], &[1072]),
    ("x=(a &|)\n", &[1009, 1073, 1072], &[1009, 1073, 1072]),
];

#[test]
fn c02_disown_ampersand_parses_in_zsh() {
    check_positives(C02_POSITIVES);
}

#[test]
fn c02_disown_ampersand_bash_output_is_unchanged() {
    check_bash_unchanged(C02_POSITIVES);
    check_bash_unchanged(C02_NEAR_MISSES);
}

#[test]
fn c02_disown_ampersand_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C02_NEAR_MISSES);
}

// ---- C03: equals process substitution -----------------------------
const C03_POSITIVES: &[Case] = &[
    ("diff =(echo a) =(echo b)\n", &[], &[2283, 1036, 1088]),
    ("f() { diff =(echo a) \"$1\"; }\nf x\n", &[], &[1009, 1073, 1036, 1056, 1072]),
    ("cat =(echo hi) | wc -l\n", &[], &[2283, 1036, 1088]),
    ("diff <(echo a) =(echo b)\n", &[], &[1036, 1088]),
    ("x=$(cat =(echo a))\necho \"$x\"\n", &[], &[1009, 1073, 1036, 1072]),
    ("cat =(\n  echo a\n  echo b\n)\n", &[], &[2283, 1036, 1088]),
    ("for f in =(echo a) =(echo b); do echo \"$f\"; done\n", &[], &[1073, 1036, 1058, 1072]),
];
const C03_NEAR_MISSES: &[Case] = &[
    ("cat =(echo a\n", &[1009, 1073, 1072], &[2283, 1036, 1088]),
    ("cat =(echo a ; \n", &[1009, 1073, 1072], &[2283, 1036, 1088]),
];

#[test]
fn c03_equals_process_substitution_parses_in_zsh() {
    check_positives(C03_POSITIVES);
}

#[test]
fn c03_equals_process_substitution_bash_output_is_unchanged() {
    check_bash_unchanged(C03_POSITIVES);
    check_bash_unchanged(C03_NEAR_MISSES);
}

#[test]
fn c03_equals_process_substitution_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C03_NEAR_MISSES);
}

// ---- C10: always block --------------------------------------------
const C10_POSITIVES: &[Case] = &[
    ("{ :; } always { :; }\n", &[], &[1070, 1141]),
    ("{\n  echo try\n} always {\n  echo cleanup\n}\n", &[], &[1070, 1141]),
    ("f() { { echo a } always { echo b } }\nf\n", &[], &[1009, 1073, 1083, 1083, 1083, 1083, 1056, 1072]),
    ("{ :; } always { :; } > /dev/null\n", &[], &[1070, 1141]),
    ("{ :; } always { :; } &\n", &[], &[1070, 1141]),
    ("{ :; } always\n{ :; }\n", &[], &[1070, 1141]),
    ("{ { :; } always { :; } } always { :; }\n", &[], &[1073, 1056, 1072, 1141]),
];
const C10_NEAR_MISSES: &[Case] = &[
    ("{ :; } always echo hi\n", &[1070, 1141], &[1070, 1141]),
    ("{ :; }\nalways { :; }\n", &[1083, 1089], &[1083, 1089]),
    ("{ :; } always { :; } always { :; }\n", &[1070, 1141], &[1070, 1141]),
];

#[test]
fn c10_always_block_parses_in_zsh() {
    check_positives(C10_POSITIVES);
}

#[test]
fn c10_always_block_bash_output_is_unchanged() {
    check_bash_unchanged(C10_POSITIVES);
    check_bash_unchanged(C10_NEAR_MISSES);
}

#[test]
fn c10_always_block_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C10_NEAR_MISSES);
}

// ---- C11: close brace without semicolon ---------------------------
const C11_POSITIVES: &[Case] = &[
    ("{ echo hi }\n", &[], &[1073, 1083, 1056, 1072]),
    ("f() { echo hi }\nf\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("f() { echo hi } ; f\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("if true; then { echo hi }; fi\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("{ echo \"}\" }\n", &[], &[1073, 1083, 1056, 1072]),
    ("f() { echo \"${HOME}\" }\nf\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("{ { echo hi }}\n", &[], &[1009, 1073, 1083, 1083, 1056, 1072]),
    ("f() { echo a; echo b }\nf\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("{echo hi}\n", &[], &[1073, 1054, 1083, 1056, 1072]),
    ("{ x=1 }\necho \"$x\"\n", &[], &[1073, 1083, 1056, 1072]),
    ("f() { [[ -n $1 ]] }\nf a\n", &[], &[1083]),
    ("{ echo hi } > /dev/null\n", &[], &[1073, 1083, 1056, 1072]),
    ("{ echo \\} }\n", &[], &[1073, 1083, 1056, 1072]),
];
const C11_NEAR_MISSES: &[Case] = &[
    ("echo hi }\n", &[1089], &[1083]),
    ("{ echo } }\n", &[1089], &[1073, 1083, 1083, 1056, 1072]),
    ("{ echo hi }}\n", &[1089], &[1073, 1083, 1083, 1056, 1072]),
    ("{ echo x} y }\n", &[1070, 1141], &[1073, 1083, 1083, 1056, 1072]),
];

#[test]
fn c11_close_brace_without_semicolon_parses_in_zsh() {
    check_positives(C11_POSITIVES);
}

#[test]
fn c11_close_brace_without_semicolon_bash_output_is_unchanged() {
    check_bash_unchanged(C11_POSITIVES);
    check_bash_unchanged(C11_NEAR_MISSES);
}

#[test]
fn c11_close_brace_without_semicolon_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C11_NEAR_MISSES);
}

// ---- C12: empty body ----------------------------------------------
const C12_POSITIVES: &[Case] = &[
    ("f() { }\nf\n", &[], &[1009, 1073, 1055, 1072]),
    ("f() {}\nf\n", &[], &[1009, 1073, 1054, 1055, 1072]),
    ("{ }\n", &[], &[1073, 1055, 1072]),
    ("function f { }\nf\n", &[], &[1009, 1073, 1055, 1072]),
    ("{ ; }\n", &[], &[1073, 1072]),
    ("if true; then { }; fi\n", &[], &[1009, 1073, 1055, 1072]),
    ("f() {\n  # nothing yet\n}\nf\n", &[], &[1009, 1073, 1055, 1072]),
    ("f() { { } }\nf\n", &[], &[1009, 1073, 1055, 1072]),
    ("function f() {}\nf\n", &[], &[1009, 1073, 1054, 1055, 1072]),
    ("{} > /dev/null\n", &[], &[1073, 1054, 1055, 1072]),
];
const C12_NEAR_MISSES: &[Case] =
    &[("f() {\n", &[1009, 1073, 1072], &[1009, 1073, 1072]), ("{ \n", &[1073, 1072], &[1073, 1072])];

#[test]
fn c12_empty_body_parses_in_zsh() {
    check_positives(C12_POSITIVES);
}

#[test]
fn c12_empty_body_bash_output_is_unchanged() {
    check_bash_unchanged(C12_POSITIVES);
    check_bash_unchanged(C12_NEAR_MISSES);
}

#[test]
fn c12_empty_body_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C12_NEAR_MISSES);
}

// ---- C13: multi name function -------------------------------------
const C13_POSITIVES: &[Case] = &[
    ("function a b { echo hi }\na\nb\n", &[], &[1073, 1064, 1072]),
    ("function a b c { echo hi }\na\nb\nc\n", &[], &[1073, 1064, 1072]),
    ("function a b() { echo hi }\na\nb\n", &[], &[1073, 1064, 1072]),
    ("a b() { echo hi }\na\nb\n", &[], &[1036, 1088]),
    ("function a b\n{ echo hi }\na\nb\n", &[], &[1073, 1064, 1072]),
    ("function a b { echo \"$1\"; }\na x\nb y\n", &[], &[1073, 1064, 1072]),
];
const C13_NEAR_MISSES: &[Case] = &[
    ("function a;b { :; }\n", &[1073, 1064, 1072], &[1073, 1064, 1072]),
    ("function a() b { :; }\n", &[1073, 1064, 1072], &[1073, 1064, 1072]),
    ("function a b {\n", &[1073, 1064, 1072, 1009], &[1073, 1064, 1072]),
];

#[test]
fn c13_multi_name_function_parses_in_zsh() {
    check_positives(C13_POSITIVES);
}

#[test]
fn c13_multi_name_function_bash_output_is_unchanged() {
    check_bash_unchanged(C13_POSITIVES);
    check_bash_unchanged(C13_NEAR_MISSES);
}

#[test]
fn c13_multi_name_function_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C13_NEAR_MISSES);
}

// ---- C14: odd function name ---------------------------------------
const C14_POSITIVES: &[Case] = &[
    ("function ${=V} { echo hi }\n", &[], &[2086, 1083, 1083]),
    ("function foo\"${1:-}\" { echo hi }\n", &[], &[1073, 1064, 1072]),
    ("function $0_x { echo hi }\n", &[], &[2086, 1083, 1083]),
    ("function {a,b}_x { echo hi }\n", &[], &[1083, 1083]),
    ("function cfh~ { echo hi }\n", &[], &[1073, 1064, 1072]),
    ("cfh~() { echo hi }\n", &[], &[1036, 1088]),
    ("$0_x() { echo hi }\n", &[], &[2086, 1036, 1088]),
    ("foo\"${1:-}\"() { echo hi }\n", &[], &[1036, 1088]),
    ("function +x { echo hi }\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("function foo\"${1:-}\"() { echo hi }\n", &[], &[1073, 1064, 1072]),
];
const C14_NEAR_MISSES: &[Case] = &[
    ("{a,b}_x() { :; }\n", &[1036, 1088], &[1073, 1054, 1083, 1036, 1056, 1072]),
    ("function foo\"${1:-}\" {\n", &[1073, 1064, 1072, 1009], &[1073, 1064, 1072]),
];

#[test]
fn c14_odd_function_name_parses_in_zsh() {
    check_positives(C14_POSITIVES);
}

#[test]
fn c14_odd_function_name_bash_output_is_unchanged() {
    check_bash_unchanged(C14_POSITIVES);
    check_bash_unchanged(C14_NEAR_MISSES);
}

#[test]
fn c14_odd_function_name_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C14_NEAR_MISSES);
}

// ---- C15: empty then else elif body -------------------------------
const C15_POSITIVES: &[Case] = &[
    ("if true; then # c\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("if true; then\n  # nothing\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("if true; then :; else # c\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("if true; then :; elif true; then # c\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("if true; then\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("if true; then; fi\n", &[], &[1009, 1073, 1051, 1048, 1072]),
    ("if true; then :; else\nfi\n", &[], &[1009, 1073, 1048, 1072]),
    ("f() {\n  if [[ -n $1 ]]; then\n    # TODO\n  fi\n}\nf a\n", &[], &[1009, 1073, 1048, 1072]),
];
const C15_NEAR_MISSES: &[Case] = &[
    ("if true; then # c\n", &[1009, 1073, 1072], &[1009, 1073, 1072]),
    ("if true; then :; else # c\n", &[1009, 1073, 1072], &[1009, 1073, 1072]),
    ("if true # c\nfi\n", &[1049, 1073, 1050, 1072], &[1049, 1073, 1050, 1072]),
];

#[test]
fn c15_empty_then_else_elif_body_parses_in_zsh() {
    check_positives(C15_POSITIVES);
}

#[test]
fn c15_empty_then_else_elif_body_bash_output_is_unchanged() {
    check_bash_unchanged(C15_POSITIVES);
    check_bash_unchanged(C15_NEAR_MISSES);
}

#[test]
fn c15_empty_then_else_elif_body_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C15_NEAR_MISSES);
}

// ---- C16: brace form if for ---------------------------------------
const C16_POSITIVES: &[Case] = &[
    ("if (( 1 )) { echo y }\n", &[], &[1073, 1050, 1072]),
    ("if [[ -n $HOME ]] { echo y } else { echo n }\n", &[], &[1073, 1050, 1140, 1083, 1072]),
    ("for i in a b; { echo \"$i\" }\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("for ((i = 0; i < 3; i++)) { echo \"$i\" }\n", &[], &[1009, 1073, 1083, 1056, 1072]),
    ("if (( 1 )) { :; } elif (( 2 )) { :; } else { :; }\n", &[], &[1073, 1050, 1072]),
    ("while [[ -n $HOME ]] { break }\n", &[], &[1073, 1058, 1140, 1083, 1072]),
    (
        "f() {\n  if (( $# > 1 )) {\n    echo many\n  } else {\n    echo few\n  }\n}\nf a\n",
        &[],
        &[1009, 1073, 1050, 1072],
    ),
    ("if (( 1 )) { }\n", &[], &[1073, 1050, 1072]),
];
const C16_NEAR_MISSES: &[Case] = &[
    ("if true { :; }\n", &[1073, 1083, 1050, 1072], &[1073, 1083, 1050, 1072]),
    ("while true { break }\n", &[1073, 1083, 1058, 1072], &[1073, 1083, 1083, 1058, 1072]),
    ("if (( 1 ))\n{ :; }\n", &[1073, 1050, 1072], &[1073, 1050, 1072]),
    ("if (( 1 )) { :;\n", &[1009, 1073, 1056, 1072], &[1073, 1050, 1072]),
];

#[test]
fn c16_brace_form_if_for_parses_in_zsh() {
    check_positives(C16_POSITIVES);
}

#[test]
fn c16_brace_form_if_for_bash_output_is_unchanged() {
    check_bash_unchanged(C16_POSITIVES);
    check_bash_unchanged(C16_NEAR_MISSES);
}

#[test]
fn c16_brace_form_if_for_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C16_NEAR_MISSES);
}

// ---- C22: anonymous function in process substitution --------------
const C22_POSITIVES: &[Case] = &[
    ("cat <(() { echo a })\n", &[], &[1009, 1073, 1072]),
    ("cat <(() { echo \"$1\" } b)\n", &[], &[1009, 1073, 1072]),
    ("diff <(() { echo a }) <(() { echo b })\n", &[], &[1009, 1073, 1072]),
    ("cat <(function { echo a })\n", &[], &[1083, 1083]),
    ("cat =(() { echo a })\n", &[], &[2283, 1036, 1088]),
    ("while read -r l; do echo \"$l\"; done < <(() { echo a })\n", &[], &[1009, 1073, 1072]),
];
const C22_NEAR_MISSES: &[Case] = &[
    ("cat <(() { echo a )\n", &[1073, 1072, 1009, 1056], &[1009, 1073, 1072]),
    ("cat <(() { echo a }\n", &[1009, 1073, 1072], &[1009, 1073, 1072]),
];

#[test]
fn c22_anonymous_function_in_process_substitution_parses_in_zsh() {
    check_positives(C22_POSITIVES);
}

#[test]
fn c22_anonymous_function_in_process_substitution_bash_output_is_unchanged() {
    check_bash_unchanged(C22_POSITIVES);
    check_bash_unchanged(C22_NEAR_MISSES);
}

#[test]
fn c22_anonymous_function_in_process_substitution_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C22_NEAR_MISSES);
}

// ---- C25: anonymous function args and disown ----------------------
const C25_POSITIVES: &[Case] = &[
    ("() { echo \"$1\" } \"$HOME\" &|\n", &[], &[1073, 1072]),
    ("() { echo \"$1\" } a b &!\n", &[], &[1073, 1072]),
    ("() { echo \"$1\" } a &\n", &[], &[1073, 1072]),
    ("() { :; } 2>&1 &|\n", &[], &[1073, 1072]),
    ("() {\n  echo \"$1\" \"$2\"\n} a b &|\n", &[], &[1073, 1072]),
    ("f() { () { echo \"$1\" } \"$1\" &| }\nf x\n", &[], &[1009, 1073, 1072]),
    ("function { echo \"$1\" } a &|\n", &[], &[1083, 1083, 1070]),
    ("() { :; } > /dev/null &|\n", &[], &[1073, 1072]),
];
const C25_NEAR_MISSES: &[Case] = &[
    ("function f() { :; } a\n", &[1070, 1141], &[1070, 1141]),
    ("f() { :; } a\n", &[1070, 1141], &[1070, 1141]),
    ("() { :; } a &||\n", &[1070], &[1073, 1072]),
];

#[test]
fn c25_anonymous_function_args_and_disown_parses_in_zsh() {
    check_positives(C25_POSITIVES);
}

#[test]
fn c25_anonymous_function_args_and_disown_bash_output_is_unchanged() {
    check_bash_unchanged(C25_POSITIVES);
    check_bash_unchanged(C25_NEAR_MISSES);
}

#[test]
fn c25_anonymous_function_args_and_disown_near_misses_are_still_errors_in_zsh() {
    check_near_misses(C25_NEAR_MISSES);
}
