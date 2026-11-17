// SPDX-License-Identifier: GPL-3.0-or-later
// Needs a real zsh, which Windows runners don't have.
#![cfg(unix)]
// DIVERGENCE: shelllint-only tests for zsh message text and gating (SC2206, SC2207, SC2302).
//
// Every zsh claim is checked against real zsh 5.9 in the same test.
#[path = "zsh_msg_common.rs"]
mod zsh_msg_common;
use zsh_msg_common::*;

const MSG_2207_ZSH: &str = "Prefer ${(f)\"$(cmd)\"} or read -rA to split command output (or quote to avoid splitting).";
const MSG_2207_BASH: &str = "Prefer mapfile or read -a to split command output (or quote to avoid splitting).";
const MSG_2207_KSH: &str = "Prefer read -A or while read to split command output (or quote to avoid splitting).";
const MSG_2206_ZSH: &str = "Quote to prevent word splitting/globbing, or split robustly with ${(s: :)var} or read -rA.";
const MSG_2206_BASH: &str = "Quote to prevent word splitting/globbing, or split robustly with mapfile or read -a.";

// ---- SC2207 (class a: still fires, zsh wording) ----

#[test]
fn sc2207_zsh_still_fires_with_zsh_wording() {
    let f = zsh("a=($(printf 'a b\\nc'))\n");
    assert_eq!(lane(&f), vec![2207]);
    assert_eq!(message_of(&f, 2207), MSG_2207_ZSH);
    // evidence: zsh does split `$(cmd)` by default (3 words from 2 lines)
    assert_eq!(zsh_run("a=($(printf 'a b\\nc')); print ${#a}"), "3");
}

#[test]
fn sc2207_bash_and_ksh_text_unchanged_next_to_zsh() {
    assert_eq!(message_of(&bash("a=($(echo x))\n"), 2207), MSG_2207_BASH);
    assert_eq!(message_of(&ksh("a=($(echo x))\n"), 2207), MSG_2207_KSH);
}

#[test]
fn sc2207_zsh_fires_on_backticks_and_in_functions() {
    assert_eq!(lane(&zsh("a=(`date`)\n")), vec![2207]);
    assert_eq!(lane(&zsh("f() {\n  local -a a\n  a=($(date))\n}\n")), vec![2207]);
}

#[test]
fn sc2207_zsh_near_miss_quoted_substitution_is_silent() {
    assert_eq!(lane(&zsh("a=(\"$(date)\")\n")), Vec::<u32>::new());
}

#[test]
fn sc2207_zsh_suggestion_parses_and_splits_on_lines() {
    // The text suggests ${(f)"$(cmd)"}: it parses, and gives 2 elements for 2 lines.
    zsh_parses("a=(${(f)\"$(printf 'a b\\nc')\"})");
    assert_eq!(zsh_run("a=(${(f)\"$(printf 'a b\\nc')\"}); print ${#a}"), "2");
    assert_eq!(zsh_run("a=(${(f)\"$(printf 'a b\\nc')\"}); print -r -- $a[1]"), "a b");
}

#[test]
fn sc2207_zsh_suggestion_read_ra_works() {
    zsh_parses("read -rA a <<< \"$(printf 'a b c')\"");
    assert_eq!(zsh_run("read -rA a <<< 'a b c'; print ${#a}"), "3");
}

#[test]
fn sc2207_zsh_suggested_form_is_not_reported_again() {
    assert_eq!(lane(&zsh("a=(${(f)\"$(date)\"})\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("a=(\"${(@f)$(date)}\")\n")), Vec::<u32>::new());
}

#[test]
fn sc2207_zsh_suggested_form_stays_silent_under_shwordsplit() {
    // evidence: with shwordsplit on, ${(f)x} still gives 2 elements (no re-splitting)
    assert_eq!(zsh_run("setopt shwordsplit; x=$(printf 'a b\\nc'); a=(${(f)x}); print ${#a}"), "2");
    assert!(lane(&zsh("setopt shwordsplit\nx=$(date)\na=(${(f)x})\n")).is_empty());
}

// ---- SC2206 (class b: shwordsplit or ${=x}) ----

#[test]
fn sc2206_zsh_default_is_silent_because_zsh_does_not_split() {
    assert_eq!(lane(&zsh("x='a b'\na=($x)\n")), Vec::<u32>::new());
    // evidence
    assert_eq!(zsh_run("x='a b'; a=($x); print ${#a}"), "1");
}

#[test]
fn sc2206_bash_next_to_zsh_default_fires_with_bash_text() {
    let f = bash("x='a b'\na=($x)\n");
    assert_eq!(lane(&f), vec![2206]);
    assert_eq!(message_of(&f, 2206), MSG_2206_BASH);
}

#[test]
fn sc2206_zsh_explicit_equals_form_fires_with_zsh_wording() {
    let f = zsh("x='a b'\na=(${=x})\n");
    assert_eq!(lane(&f), vec![2206]);
    assert_eq!(message_of(&f, 2206), MSG_2206_ZSH);
    // evidence: ${=x} splits with the default options
    assert_eq!(zsh_run("x='a b'; a=(${=x}); print ${#a}"), "2");
}

#[test]
fn sc2206_zsh_explicit_equals_form_fires_in_function_and_with_other_elements() {
    assert_eq!(lane(&zsh("f() {\n  local x='a b'\n  a=(1 ${=x} 2)\n}\n")), vec![2206]);
}

#[test]
fn sc2206_zsh_non_splitting_conditions_still_apply() {
    // counting reference, quoted form: silent even with the explicit splitting spelling absent
    assert_eq!(lane(&zsh("a=(${#x})\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("a=(\"${=x}\")\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("a=($#)\n")), Vec::<u32>::new());
}

#[test]
fn sc2206_zsh_explicit_splitting_flags_are_not_reported() {
    // evidence: with shwordsplit on, (f) and (s) results are not split again
    assert_eq!(zsh_run("setopt shwordsplit; x='a:b c'; a=(${(s.:.)x}); print ${#a}"), "2");
    assert_eq!(zsh_run("setopt shwordsplit; x=$'a b\\nc'; a=(${(f)x}); print ${#a}"), "2");
    assert_eq!(lane(&zsh("x='a:b c'\na=(${(s.:.)x})\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("x='a b'\na=(${(f)x})\n")), Vec::<u32>::new());
}

#[test]
fn sc2206_zsh_suggested_split_forms_parse_and_split() {
    zsh_parses("x='a:b'; a=(${(s.:.)x})");
    assert_eq!(zsh_run("x='a:b'; a=(${(s: :)x}); print ${#a}"), "1");
    assert_eq!(zsh_run("x='a b'; a=(${(s: :)x}); print ${#a}"), "2");
}

#[test]
fn sc2206_zsh_fires_with_shwordsplit_on() {
    assert_eq!(zsh_run("setopt shwordsplit; x='a b'; a=($x); print ${#a}"), "2");
    let f = zsh("setopt shwordsplit\nx='a b'\na=($x)\n");
    assert_eq!(lane(&f), vec![2206]);
    assert_eq!(message_of(&f, 2206), MSG_2206_ZSH);
}

#[test]
fn sc2206_zsh_silent_after_unsetopt_shwordsplit_and_inside_scoped_function() {
    assert!(lane(&zsh("setopt shwordsplit\nunsetopt shwordsplit\nx='a b'\na=($x)\n")).is_empty());
    assert!(lane(&zsh("f() {\n  setopt localoptions shwordsplit\n}\nx='a b'\na=($x)\n")).is_empty());
    assert_eq!(lane(&zsh("emulate sh\nx='a b'\na=($x)\n")), vec![2206]);
}

// ---- SC2302 (loop over values) ----

const LOOP: &str = "for i in \"${a[@]}\"; do\n  echo \"${a[$i]}\"\ndone\n";

#[test]
fn sc2302_zsh_text_names_the_zsh_key_expansion() {
    let f = zsh(LOOP);
    assert_eq!(lane(&f), vec![2302, 2303]);
    assert_eq!(
        message_of(&f, 2302),
        "This loops over values. To loop over keys, use \"${(@k)array}\" (associative) or {1..$#array}."
    );
    // the 2303 text is shell-neutral and unchanged
    assert_eq!(message_of(&f, 2303), message_of(&bash(LOOP), 2303));
}

#[test]
fn sc2302_bash_text_unchanged() {
    assert_eq!(message_of(&bash(LOOP), 2302), "This loops over values. To loop over keys, use \"${!array[@]}\".");
}

#[test]
fn sc2302_zsh_suggestions_are_valid_zsh_and_give_keys() {
    // `${!array[@]}` (the bash advice) is not valid zsh; the zsh forms are.
    assert!(zsh_run("a=(x y); print \"${!a[@]}\"").contains("bad substitution"));
    zsh_parses("typeset -A m; for k in \"${(@k)m}\"; do :; done");
    assert_eq!(
        zsh_run("typeset -A m; m=('a b' 1 c 2); for k in \"${(@k)m}\"; do print -r -- \"<$k>\"; done"),
        "<a b>\n<c>"
    );
    assert_eq!(zsh_run("a=(x y z); print {1..$#a}"), "1 2 3");
}
