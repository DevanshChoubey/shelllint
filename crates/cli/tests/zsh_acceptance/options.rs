// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// The zsh option rules of the design (spelling, scope, emulate), observed through what
// the checks report. The main probe is SC2086 on `echo $1`: it must fire exactly where
// the option state in effect makes zsh split `$1`.
//
// Every `split_case!` has two tests:
//   * `shelllint_reports_these_lines`: SC2086 appears on exactly the listed lines.
//   * `real_zsh_splits_at_these_lines`: real zsh 5.9 splits `$1` on exactly those lines
//     (the script is run with each `echo $1` replaced by a word counter). This always
//     runs (skipped without zsh), so the expectation is zsh's, not ours. Two cases are
//     "definition site" cases that follow the design rule "a function sees the options
//     in effect where it is defined", which zsh's dynamic scope does not; they have no
//     oracle test.
//
// Spelling rules checked with zsh: case-insensitive, underscores ignored, `no` prefix
// negates, `unsetopt noX` sets X, `set -o/+o`, `setopt -o/+o`, `options[x]=on|off`.
// NOTE: the design (and research) say `options[noX]` is not honoured. Real zsh 5.9
// honours it (`options[noshwordsplit]=on` turns shwordsplit off), so there is no test
// either way; see the report.
use crate::support::*;

fn split_lint(script: &str, expect: &[u32]) {
    let got = zsh(script);
    assert_eq!(lines(&got, 2086), expect, "SC2086 lines under zsh (all findings {:?}) for:\n{script}", pairs(&got));
}

fn split_oracle(script: &str, expect: &[u32]) {
    if !have_zsh() {
        eprintln!("zsh not installed: skipping");
        return;
    }
    assert_eq!(zsh_split_lines(script), expect, "real zsh splits `$1` at these lines for:\n{script}");
}

macro_rules! split_case {
    (now, $n:ident, $s:expr, $e:expr, $oracle:expr) => {
        mod $n {
            use super::*;
            #[test]
            fn shelllint_reports_these_lines() {
                split_lint($s, &$e)
            }
            #[test]
            fn real_zsh_splits_at_these_lines() {
                if $oracle {
                    split_oracle($s, &$e)
                }
            }
        }
    };
}

split_case!(now, default_does_not_split, "f() { echo $1; }\nf \"a b\"\n", [], true);
split_case!(now, top_level_default_does_not_split, "echo $1\n", [], true);
split_case!(now, setopt_shwordsplit_turns_it_on, "setopt shwordsplit\nf() { echo $1; }\nf \"a b\"\n", [2], true);
split_case!(now, spell_on__lower, "setopt shwordsplit\necho $1\n", [2], true);
split_case!(now, spell_on__upper, "setopt SHWORDSPLIT\necho $1\n", [2], true);
split_case!(now, spell_on__underscored_upper, "setopt SH_WORD_SPLIT\necho $1\n", [2], true);
split_case!(now, spell_on__underscored_lower, "setopt sh_word_split\necho $1\n", [2], true);
split_case!(now, spell_on__mixed_case, "setopt Sh_Word_Split\necho $1\n", [2], true);
split_case!(now, spell_on__unsetopt_no_prefix, "unsetopt noshwordsplit\necho $1\n", [2], true);
split_case!(now, spell_on__unsetopt_no_underscore_prefix, "unsetopt no_sh_word_split\necho $1\n", [2], true);
split_case!(now, spell_on__unsetopt_upper_no, "unsetopt NO_SH_WORD_SPLIT\necho $1\n", [2], true);
split_case!(now, spell_on__set_dash_o, "set -o shwordsplit\necho $1\n", [2], true);
split_case!(now, spell_on__set_dash_o_underscored, "set -o sh_word_split\necho $1\n", [2], true);
split_case!(now, spell_on__setopt_dash_o, "setopt -o shwordsplit\necho $1\n", [2], true);
split_case!(now, spell_on__options_array_on, "options[shwordsplit]=on\necho $1\n", [2], true);
split_case!(now, spell_on__options_array_upper_underscored, "options[SH_WORD_SPLIT]=on\necho $1\n", [2], true);
split_case!(now, spell_on__emulate_sh, "emulate sh\necho $1\n", [2], true);
split_case!(now, spell_on__emulate_ksh, "emulate ksh\necho $1\n", [2], true);
split_case!(now, spell_on__emulate_R_sh, "emulate -R sh\necho $1\n", [2], true);
split_case!(now, spell_on__emulate_L_zsh_dash_o, "emulate -L zsh -o shwordsplit\necho $1\n", [2], true);
split_case!(now, spell_on__among_other_options, "setopt extendedglob shwordsplit nomatch\necho $1\n", [2], true);
split_case!(now, spell_on__two_setopts, "setopt extendedglob; setopt shwordsplit\necho $1\n", [2], true);
split_case!(now, spell_off__noshwordsplit, "setopt noshwordsplit\necho $1\n", [], true);
split_case!(now, spell_off__on_then_noshwordsplit, "setopt shwordsplit; setopt noshwordsplit\necho $1\n", [], true);
split_case!(now, spell_off__on_then_no_underscore, "setopt shwordsplit; setopt no_sh_word_split\necho $1\n", [], true);
split_case!(now, spell_off__on_then_unsetopt, "setopt shwordsplit; unsetopt shwordsplit\necho $1\n", [], true);
split_case!(now, spell_off__on_then_unsetopt_upper, "setopt shwordsplit; unsetopt SH_WORD_SPLIT\necho $1\n", [], true);
split_case!(now, spell_off__on_then_set_plus_o, "setopt shwordsplit; set +o shwordsplit\necho $1\n", [], true);
split_case!(now, spell_off__on_then_setopt_plus_o, "setopt shwordsplit; setopt +o shwordsplit\necho $1\n", [], true);
split_case!(
    now,
    spell_off__on_then_options_array_off,
    "setopt shwordsplit; options[shwordsplit]=off\necho $1\n",
    [],
    true
);
split_case!(now, spell_off__on_then_emulate_zsh, "setopt shwordsplit; emulate zsh\necho $1\n", [], true);
split_case!(now, spell_off__on_then_emulate_L_zsh, "setopt shwordsplit; emulate -L zsh\necho $1\n", [], true);
split_case!(now, spell_off__on_then_emulate_R_zsh, "setopt shwordsplit; emulate -R zsh\necho $1\n", [], true);
split_case!(now, spell_off__same_command_later_wins, "setopt shwordsplit noshwordsplit\necho $1\n", [], true);
// `emulate sh` also turns globsubst on, which keeps SC2086 true for the glob half; `-o noglobsubst` isolates splitting.
split_case!(
    now,
    spell_off__emulate_sh_then_no_option,
    "emulate sh -o noshwordsplit -o noglobsubst\necho $1\n",
    [],
    true
);
split_case!(now, spell_off__unrelated_extendedglob, "setopt extendedglob\necho $1\n", [], true);
split_case!(now, spell_off__unrelated_noksharrays, "setopt noksharrays\necho $1\n", [], true);
split_case!(now, spell_off__unrelated_unsetopt_nomatch, "unsetopt nomatch\necho $1\n", [], true);
split_case!(now, spell_on_survives_unrelated_option, "setopt shwordsplit; setopt extendedglob\necho $1\n", [2], true);
split_case!(
    now,
    scope__function_without_localoptions_leaks,
    "f() { setopt shwordsplit; }\nf \"a b\"\necho $1\n",
    [3],
    true
);
split_case!(
    now,
    scope__localoptions_restores_on_return,
    "f() { setopt localoptions shwordsplit; echo $1; }\nf \"a b\"\necho $1\n",
    [1],
    true
);
split_case!(
    now,
    scope__localoptions_given_after_the_change,
    "f() { setopt shwordsplit; setopt localoptions; echo $1; }\nf \"a b\"\necho $1\n",
    [1],
    true
);
split_case!(
    now,
    scope__localoptions_given_after_the_use,
    "f() { setopt shwordsplit; echo $1; setopt localoptions; }\nf \"a b\"\necho $1\n",
    [1],
    true
);
split_case!(
    now,
    scope__localoptions_spelled_local_options_upper,
    "f() { setopt LOCAL_OPTIONS shwordsplit; echo $1; }\nf \"a b\"\necho $1\n",
    [1],
    true
);
split_case!(
    now,
    scope__later_nolocaloptions_cancels_it,
    "f() { setopt localoptions shwordsplit; setopt nolocaloptions; }\nf \"a b\"\necho $1\n",
    [3],
    true
);
split_case!(
    now,
    scope__later_unsetopt_localoptions_cancels_it,
    "f() { setopt localoptions shwordsplit; unsetopt localoptions; }\nf \"a b\"\necho $1\n",
    [3],
    true
);
split_case!(
    now,
    scope__anonymous_function_with_localoptions,
    "() { setopt localoptions shwordsplit; echo $1; } \"a b\"\necho $1\n",
    [1],
    true
);
split_case!(
    now,
    scope__anonymous_function_without_localoptions_leaks,
    "() { setopt shwordsplit; }\necho $1\n",
    [2],
    true
);
split_case!(now, scope__if_does_not_scope, "if true; then setopt shwordsplit; fi\necho $1\n", [2], true);
split_case!(now, scope__for_does_not_scope, "for x in a; do setopt shwordsplit; done\necho $1\n", [2], true);
split_case!(
    now,
    scope__while_does_not_scope,
    "n=1\nwhile (( n-- )); do setopt shwordsplit; done\necho $1\n",
    [3],
    true
);
split_case!(now, scope__brace_group_does_not_scope, "{ setopt shwordsplit; }\necho $1\n", [2], true);
split_case!(
    now,
    scope__emulate_L_zsh_scopes_and_resets,
    "setopt shwordsplit\nf() { emulate -L zsh; echo $1; }\nf \"a b\"\necho $1\n",
    [4],
    true
);
split_case!(
    now,
    scope__emulate_LR_zsh_scopes_and_resets,
    "setopt shwordsplit\nf() { emulate -LR zsh; echo $1; }\nf \"a b\"\necho $1\n",
    [4],
    true
);
split_case!(
    now,
    scope__emulate_R_without_L_leaks_the_reset,
    "setopt shwordsplit\nf() { emulate -R zsh; }\nf\necho $1\n",
    [],
    true
);
split_case!(
    now,
    scope__emulate_zsh_without_L_leaks_the_reset,
    "setopt shwordsplit\nf() { emulate zsh; }\nf\necho $1\n",
    [],
    true
);
split_case!(now, scope__emulate_L_sh_in_function, "f() { emulate -L sh; echo $1; }\nf \"a b\"\necho $1\n", [1], true);
split_case!(now, scope__emulate_L_ksh_in_function, "f() { emulate -L ksh; echo $1; }\nf \"a b\"\necho $1\n", [1], true);
split_case!(now, scope__top_level_emulate_L_zsh_resets, "setopt shwordsplit\nemulate -L zsh\necho $1\n", [], true);
split_case!(now, scope__top_level_emulate_L_sh_applies, "emulate -L sh\necho $1\n", [2], true);
split_case!(
    now,
    scope__function_called_later_sees_leaked_state,
    "f() { setopt shwordsplit; }\nf\ng() { echo $1; }\ng \"a b\"\n",
    [3],
    true
);
split_case!(
    now,
    scope__sibling_function_unaffected,
    "f() { setopt localoptions shwordsplit; }\ng() { echo $1; }\nf\ng \"a b\"\n",
    [],
    true
);
split_case!(now, scope__own_setopt_before_use, "f() { setopt shwordsplit; echo $1; }\nf \"a b\"\n", [1], true);
split_case!(
    now,
    scope__nested_function_defined_under_option,
    "outer() { setopt localoptions shwordsplit; inner() { echo $1; }; inner \"a b\"; }\nouter\n",
    [1],
    true
);
split_case!(
    now,
    scope__top_level_localoptions_does_not_hide_changes,
    "setopt localoptions\nsetopt shwordsplit\necho $1\n",
    [3],
    true
);
split_case!(
    now,
    scope__definition_site__changed_after_definition_is_not_seen,
    "f() { echo $1; }\nsetopt shwordsplit\nf \"a b\"\n",
    [],
    false
);
split_case!(
    now,
    scope__definition_site__changed_back_after_definition_is_seen,
    "setopt shwordsplit\nf() { echo $1; }\nunsetopt shwordsplit\nf \"a b\"\n",
    [2],
    false
);

// ---- other options, observed through the code each one gates ------------------------------

code_case!(now, ksharrays_on__sc2128__lower, 2128, "setopt ksharrays\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__lower, 1087, "setopt ksharrays\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__upper_underscored, 2128, "setopt KSH_ARRAYS\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__upper_underscored, 1087, "setopt KSH_ARRAYS\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__underscored, 2128, "setopt ksh_arrays\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__underscored, 1087, "setopt ksh_arrays\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__unsetopt_no_prefix, 2128, "unsetopt noksharrays\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__unsetopt_no_prefix, 1087, "unsetopt noksharrays\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__set_dash_o, 2128, "set -o ksharrays\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__set_dash_o, 1087, "set -o ksharrays\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__emulate_sh, 2128, "emulate sh\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__emulate_sh, 1087, "emulate sh\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2128__emulate_ksh, 2128, "emulate ksh\na=(1 2)\necho \"$a\"\n", [3]);
code_case!(now, ksharrays_on__sc1087__emulate_ksh, 1087, "emulate ksh\na=(1 2)\necho \"$a[1]\"\n", [3]);
code_case!(now, ksharrays_on__sc2178__lower, 2178, "setopt ksharrays\na=(x y)\na=z\n", [3]);
code_case!(now, ksharrays_off__sc2128__default, 2128, "a=(1 2)\necho \"$a\"\n", []);
code_case!(now, ksharrays_off__sc1087__default, 1087, "a=(1 2)\necho \"$a[1]\"\n", []);
code_case!(now, ksharrays_off__sc2178__default, 2178, "a=(x y)\na=z\n", []);
code_case!(
    now,
    ksharrays_off__on_then_noksharrays,
    2128,
    "setopt ksharrays\nsetopt noksharrays\na=(1 2)\necho \"$a\"\n",
    []
);
code_case!(
    now,
    ksharrays_off__on_then_unsetopt,
    2128,
    "setopt ksharrays\nunsetopt ksharrays\na=(1 2)\necho \"$a\"\n",
    []
);
code_case!(
    now,
    ksharrays_off__on_then_emulate_L_zsh,
    2128,
    "setopt ksharrays\nemulate -L zsh\na=(1 2)\necho \"$a\"\n",
    []
);
code_case!(
    now,
    ksharrays_scope__localoptions_in_function_only,
    2128,
    "f() { setopt localoptions ksharrays; a=(1 2); echo \"$a\"; }\nf\nb=(1 2)\necho \"$b\"\n",
    [1]
);
code_case!(
    now,
    ksharrays_scope__leak_without_localoptions,
    2128,
    "f() { setopt ksharrays; }\nf\nb=(1 2)\necho \"$b\"\n",
    [4]
);
code_case!(now, ksharrays_not_enabled_by_shwordsplit, 2128, "setopt shwordsplit\na=(1 2)\necho \"$a\"\n", []);
code_case!(now, globsubst_on__sc2053__lower, 2053, "setopt globsubst\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2053__upper_underscored, 2053, "setopt GLOB_SUBST\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2053__underscored, 2053, "setopt glob_subst\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2053__unsetopt_no_prefix, 2053, "unsetopt noglobsubst\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2053__emulate_sh, 2053, "emulate sh\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2053__emulate_ksh, 2053, "emulate ksh\n[[ $1 == $2 ]]\n", [2]);
code_case!(now, globsubst_on__sc2295__lower, 2295, "setopt globsubst\nx=\"${1#$2}\"\n", [2]);
code_case!(now, globsubst_on__sc2254__lower, 2254, "setopt globsubst\ncase $1 in $2) :;; esac\n", [2]);
code_case!(now, globsubst_on__sc2086_glob_part__without_shwordsplit, 2086, "setopt globsubst\necho $1\n", [2]);
code_case!(now, globsubst_off__sc2053__default, 2053, "[[ $1 == $2 ]]\n", []);
code_case!(now, globsubst_off__sc2295__default, 2295, "x=\"${1#$2}\"\n", []);
code_case!(now, globsubst_off__sc2254__default, 2254, "case $1 in $2) :;; esac\n", []);
code_case!(now, globsubst_off__on_then_noglobsubst, 2053, "setopt globsubst\nsetopt noglobsubst\n[[ $1 == $2 ]]\n", []);
code_case!(now, globsubst_off__on_then_emulate_L_zsh, 2053, "setopt globsubst\nemulate -L zsh\n[[ $1 == $2 ]]\n", []);
code_case!(
    now,
    globsubst_scope__localoptions_in_function_only,
    2053,
    "f() { setopt localoptions globsubst; [[ $1 == $2 ]]; }\nf\n[[ $1 == $2 ]]\n",
    [1]
);
code_case!(
    now,
    globsubst_scope__leak_without_localoptions,
    2053,
    "f() { setopt globsubst; }\nf\n[[ $1 == $2 ]]\n",
    [3]
);
code_case!(now, globsubst_not_enabled_by_shwordsplit, 2053, "setopt shwordsplit\n[[ $1 == $2 ]]\n", []);
code_case!(now, globsubst_quoted_rhs_stays_silent, 2053, "setopt globsubst\n[[ $1 == \"$2\" ]]\n", []);
code_case!(now, bsdecho_on__sc2028__lower, 2028, "setopt bsdecho\necho \"a\\nb\"\n", [2]);
code_case!(now, bsdecho_on__sc2028__upper_underscored, 2028, "setopt BSD_ECHO\necho \"a\\nb\"\n", [2]);
code_case!(now, bsdecho_on__sc2028__underscored, 2028, "setopt bsd_echo\necho \"a\\nb\"\n", [2]);
code_case!(now, bsdecho_on__sc2028__unsetopt_no_prefix, 2028, "unsetopt nobsdecho\necho \"a\\nb\"\n", [2]);
code_case!(now, bsdecho_on__sc2028__emulate_sh, 2028, "emulate sh\necho \"a\\nb\"\n", [2]);
code_case!(now, bsdecho_off__default, 2028, "echo \"a\\nb\"\n", []);
code_case!(now, bsdecho_off__emulate_ksh_has_no_bsdecho, 2028, "emulate ksh\necho \"a\\nb\"\n", []);
code_case!(now, bsdecho_off__on_then_unsetopt, 2028, "setopt bsdecho\nunsetopt bsdecho\necho \"a\\nb\"\n", []);
code_case!(now, bsdecho_off__dash_e_never_warns, 2028, "setopt bsdecho\necho -e \"a\\nb\"\n", []);
code_case!(now, bsdecho_off__no_escape_no_warning, 2028, "setopt bsdecho\necho \"ab\"\n", []);
code_case!(
    now,
    bsdecho_scope__localoptions_in_function_only,
    2028,
    "f() { setopt localoptions bsdecho; echo \"a\\nb\"; }\nf\necho \"a\\nb\"\n",
    [1]
);
code_case!(
    now,
    promptsubst_on__single_quoted_prompt_is_fine__lower,
    2016,
    "setopt promptsubst\nPROMPT='$(branch) %# '\n",
    []
);
code_case!(
    now,
    promptsubst_on__single_quoted_prompt_is_fine__upper_underscored,
    2016,
    "setopt PROMPT_SUBST\nPROMPT='$(branch) %# '\n",
    []
);
code_case!(
    now,
    promptsubst_on__single_quoted_prompt_is_fine__underscored,
    2016,
    "setopt prompt_subst\nPROMPT='$(branch) %# '\n",
    []
);
code_case!(
    now,
    promptsubst_on__single_quoted_prompt_is_fine__unsetopt_no_prefix,
    2016,
    "unsetopt nopromptsubst\nPROMPT='$(branch) %# '\n",
    []
);
code_case!(now, promptsubst_on__rprompt_and_ps1_too, 2016, "setopt promptsubst\nRPROMPT='${vcs}'\nPS1='$x'\n", []);
code_case!(now, promptsubst_off__default_still_warns, 2016, "PROMPT='$(branch) %# '\n", [1]);
code_case!(now, promptsubst_off__nopromptsubst, 2016, "setopt nopromptsubst\nPROMPT='$(branch) %# '\n", [2]);
code_case!(
    now,
    promptsubst_off__on_then_unsetopt,
    2016,
    "setopt promptsubst\nunsetopt promptsubst\nPROMPT='$x'\n",
    [3]
);
code_case!(now, promptsubst_on__other_variables_still_warn, 2016, "setopt promptsubst\nFOO='$x'\n", [2]);
code_case!(
    now,
    promptsubst_scope__localoptions_in_function_only,
    2016,
    "f() { setopt localoptions promptsubst; PROMPT='$x'; }\nPROMPT='$y'\n",
    [2]
);
// zsh 5.9: `setopt promptsubst; emulate -L zsh; [[ -o promptsubst ]]` is true (and the same for a
// plain `emulate zsh`); only `emulate -R zsh` switches it off again.
code_case!(
    now,
    promptsubst_scope__emulate_L_zsh_keeps_it,
    2016,
    "setopt promptsubst\nemulate -L zsh\nPROMPT='$x'\n",
    []
);
code_case!(
    now,
    promptsubst_scope__emulate_R_zsh_resets,
    2016,
    "setopt promptsubst\nemulate -R zsh\nPROMPT='$x'\n",
    [3]
);
