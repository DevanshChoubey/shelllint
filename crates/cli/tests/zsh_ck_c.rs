// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh work (no ShellCheck counterpart).
#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

fn has(codes: &[u32], c: u32) -> bool {
    codes.contains(&c)
}

// Test layer 4, class (c): zsh changes the trigger (special parameters, hook functions,
// builtins with different flags). Each code has a zsh case and a bash guard.
//
// zsh evidence (zsh 5.9, `/usr/bin/zsh -fc`):
//   echo $pipestatus $ZSH_VERSION $RANDOM $fpath $PROMPT   -> all set by the shell
//   typeset -A m; m[k]=v; echo ${m[k]} $m[k]                -> v v   (assoc keys are not variables)
//   precmd() { :; }                                         -> run by the shell before each prompt
//   echo ${+x}                                              -> 0 / 1 (is x set), not a use of x
//   read -k 1 c / read -q r                                 -> -k and -q read raw input

// ---- SC2154: special parameters are assigned by the shell

#[test]
fn sc2154_zsh_special_parameters_are_assigned() {
    for v in [
        "pipestatus",
        "ZSH_VERSION",
        "RANDOM",
        "fpath",
        "PROMPT",
        "precmd_functions",
        "commands",
        "functions",
        "reply",
        "REPLY",
        "path",
        "options",
    ] {
        let s = format!("echo ${{{v}}}\n");
        assert!(!has(&zsh_codes(&s), 2154), "{v}");
    }
}

#[test]
fn sc2154_bash_still_flags_zsh_names() {
    for v in ["pipestatus", "fpath", "precmd_functions"] {
        let s = format!("echo ${{{v}}}\n");
        assert!(has(&bash_codes(&s), 2154), "{v}");
    }
}

#[test]
fn sc2154_unknown_name_still_flagged_under_zsh() {
    assert!(has(&zsh_codes("echo $surely_unset_name\n"), 2154));
}

#[test]
fn sc2154_associative_array_key_is_not_a_variable() {
    assert!(!has(&zsh_codes("typeset -A m\nm[k]=v\necho ${m[k]}\n"), 2154));
    assert!(!has(&zsh_codes("typeset -A m\nm[k]=v\necho \"$m[k]\"\n"), 2154));
}

#[test]
fn sc2154_assoc_key_bash_guard() {
    assert!(has(&bash_codes("echo $a[1]\n"), 2154));
}

// ---- SC2034: the same specials are never "unused"; flag forms count as uses

#[test]
fn sc2034_flagged_reference_counts_as_use() {
    for body in ["echo ${(f)x}", "echo ${=x}", "echo ${+x}", "echo ${^x}", "echo ${~x}"] {
        let s = format!("x=1\n{body}\n");
        assert!(!has(&zsh_codes(&s), 2034), "{body}");
    }
}

#[test]
fn sc2034_real_unused_still_flagged_under_zsh() {
    assert!(has(&zsh_codes("x=1\ny=2\necho $y\n"), 2034));
}

#[test]
fn sc2034_bash_guard_plain_unused() {
    assert!(has(&bash_codes("x=1\n"), 2034));
}

#[test]
fn sc2034_assigned_special_is_not_unused() {
    assert!(!has(&zsh_codes("REPLY=1\nreply=(a)\nPS1=x\n"), 2034));
}

// ---- SC2329: hook functions are invoked by the shell

#[test]
fn sc2329_zsh_hook_functions_are_registered() {
    for h in ["precmd", "preexec", "chpwd", "periodic", "zshaddhistory", "zshexit", "TRAPINT", "TRAPEXIT", "TRAPZERR"] {
        let s = format!("{h}() {{ :; }}\nexit 0\n");
        assert!(!has(&zsh_codes(&s), 2329), "{h}");
    }
}

#[test]
fn sc2329_hook_names_are_ordinary_functions_in_bash() {
    assert!(has(&bash_codes("precmd() { :; }\nexit 0\n"), 2329));
}

#[test]
fn sc2329_ordinary_function_still_flagged_under_zsh() {
    assert!(has(&zsh_codes("foo() { :; }\nexit 0\n"), 2329));
}

#[test]
fn sc2329_function_added_to_hook_array_is_not_flagged() {
    assert!(!has(&zsh_codes("hook() { :; }\nprecmd_functions+=(hook)\nexit 0\n"), 2329));
}

// ---- SC2119 / SC2120: hooks receive arguments from the shell

#[test]
fn sc2120_hook_using_arguments_is_not_flagged() {
    assert!(!has(&zsh_codes("chpwd() { echo $1; }\nexit 0\n"), 2120));
    assert!(!has(&zsh_codes("preexec() { echo $1; }\nexit 0\n"), 2120));
}

#[test]
fn sc2120_ordinary_function_still_flagged_under_zsh() {
    assert!(has(&zsh_codes("f() { echo $1; }\nf\nexit 0\n"), 2120));
}

#[test]
fn sc2119_ordinary_call_still_flagged_under_zsh_and_bash() {
    assert!(has(&zsh_codes("f() { echo $1; }\nf\nexit 0\n"), 2119));
    assert!(has(&bash_codes("f() { echo $1; }\nf\nexit 0\n"), 2119));
}

// ---- SC2004: `$+name` is not arithmetic use

#[test]
fn sc2004_plus_name_is_not_an_arithmetic_expansion_of_x() {
    assert_eq!(zsh_codes("x=1\necho ${+x}\n"), Vec::<u32>::new());
}

// ---- SC2030 / SC2031: zsh subshell scoping matches bash for (..) groups

#[test]
fn sc2030_subshell_modification_still_reported() {
    assert!(has(&zsh_codes("x=1\n(x=2)\necho $x\n"), 2030));
    assert!(has(&bash_codes("x=1\n(x=2)\necho $x\n"), 2030));
}

#[test]
fn sc2031_use_after_subshell_still_reported() {
    assert!(has(&zsh_codes("x=1\n(x=2)\necho $x\n"), 2031));
}

#[test]
fn sc2030_brace_group_is_not_a_subshell() {
    assert!(!has(&zsh_codes("x=1\n{ x=2; }\necho $x\n"), 2030));
}

// ---- SC2162: zsh read -k / -q / -t 0

#[test]
fn sc2162_read_k_is_silent_under_zsh_and_flagged_under_bash() {
    assert!(!has(&zsh_codes("read -k 1 c\necho \"$c\"\n"), 2162));
    assert!(has(&bash_codes("read -k 1 c\necho \"$c\"\n"), 2162));
}

#[test]
fn sc2162_plain_read_still_flagged_under_zsh() {
    assert!(has(&zsh_codes("read line\necho \"$line\"\n"), 2162));
}

#[test]
fn sc2162_read_r_stays_silent() {
    assert!(!has(&zsh_codes("read -r line\necho \"$line\"\n"), 2162));
}

// ---- SC2148: the shebang rule is satisfied by the file's type

#[test]
fn sc2148_zsh_file_name_needs_no_shebang() {
    // zsh_codes selects zsh explicitly, so a missing shebang is not an error.
    assert!(!has(&zsh_codes("echo hi\n"), 2148));
}

// ---- SC2221 / SC2222 / SC2195: case patterns, conservative gating

#[test]
fn sc2221_ordinary_overlap_still_reported_under_zsh() {
    assert!(
        has(&zsh_codes("case $1 in a*) :;; ab) :;; esac\n"), 2221)
            || has(&zsh_codes("case $1 in a*) :;; ab) :;; esac\n"), 2222)
    );
}

#[test]
#[ignore = "zsh: pending P-B (extended-glob pattern syntax)"]
fn sc2195_zsh_pattern_forms_are_not_flagged() {
    assert_eq!(zsh_codes("setopt extendedglob\ncase $1 in (a|b)#) :;; esac\n"), Vec::<u32>::new());
}
