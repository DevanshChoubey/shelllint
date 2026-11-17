// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh work (no ShellCheck counterpart).
//
// Test layer 4, class (d): bash-only rules that are wrong in zsh, so silent under zsh
// and unchanged under bash.
//
// zsh evidence (zsh 5.9, `/usr/bin/zsh -fc`):
//   typeset -A m=(k v); echo ${m[k]}      -> v          (key value pairs are valid)
//   n=3; echo {1..$n}                     -> 1 2 3      (variables work in brace ranges)
//   [[ abc =~ "b" ]]                      -> matches    (a quoted regex is fine)
#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

fn has(codes: &[u32], c: u32) -> bool {
    codes.contains(&c)
}

#[test]
fn sc2190_assoc_pairs_silent_under_zsh_fires_under_bash() {
    let s = "declare -A m=(k v)\necho \"${m[k]}\"\n";
    assert!(!has(&zsh_codes(s), 2190));
    assert!(has(&bash_codes(s), 2190));
}

#[test]
fn sc2051_variable_in_brace_range_silent_under_zsh_fires_under_bash() {
    let s = "n=3\necho {1..$n}\n";
    assert!(!has(&zsh_codes(s), 2051));
    assert!(has(&bash_codes(s), 2051));
}

#[test]
fn sc2180_second_subscript_silent_under_zsh_fires_under_bash() {
    let s = "a=(1 2)\necho \"${a[1][2]}\"\n";
    assert!(!has(&zsh_codes(s), 2180));
    assert!(has(&bash_codes(s), 2180));
}

#[test]
fn sc2076_quoted_regex_silent_under_zsh() {
    let s = "[[ abc =~ \"b.c\" ]]\n";
    assert!(!has(&zsh_codes(s), 2076));
}

#[test]
fn sc2076_bash_unchanged() {
    assert_same_as_bash_today("[[ abc =~ \"b.c\" ]]\n", &[2050, 2076]);
}

#[test]
fn sc2168_local_outside_function_silent_under_zsh() {
    // `local` at top level is accepted by zsh.
    assert!(!has(&zsh_codes("local x=1\necho \"$x\"\n"), 2168));
}

#[test]
fn sc2168_bash_unchanged() {
    assert!(has(&bash_codes("local x=1\necho \"$x\"\n"), 2168));
}

#[test]
fn sc2277_and_sc2338_are_not_reported_under_zsh() {
    assert!(!has(&zsh_codes("[ a == b ]\n"), 2277));
    assert!(!has(&zsh_codes("[ a == b ]\n"), 2338));
}

#[test]
fn d_class_does_not_hide_ordinary_findings() {
    // Near-miss: an unrelated warning on the same script is still reported.
    assert!(has(&zsh_codes("declare -A m=(k v)\ncd /tmp\n"), 2164));
}
