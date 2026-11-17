// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the version guards in `check_bash_version/guards.rs`: a test on BASH_VERSINFO or
// BASH_VERSION raises the target inside the region it protects. No Haskell counterpart.
//
// Each positive form is checked at target 3.2, where the guarded feature would otherwise warn.
// Each near miss must still warn. Forms that real bash reads as a version test are checked
// against /bin/bash in the shell that the probes used (see the report).

mod bash_version_common;
use bash_version_common::*;

use shelllint_core::interface::{Code, Shell};

// --- Positive forms: the guarded feature is silent at the old target ---------------------------

#[test]
fn guard_arith_if_silences_declare_a() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] >= 4)); then declare -A m; fi\n");
}

#[test]
fn guard_mapfile_after_double_bracket_major_from_version_string() {
    assert_silent("3.2", "[[ ${BASH_VERSION%%.*} -ge 4 ]] && mapfile -t a < f\n");
}

#[test]
fn guard_test_bracket_silences_mapfile_and_case_modification() {
    // The brief's false positive: both SC9103 and SC9107 come from the guarded region.
    assert_silent("3.2", "if [ \"${BASH_VERSINFO[0]}\" -ge 4 ]; then readarray -t a < f; echo \"${a[0]^^}\"; fi\n");
}

#[test]
fn guard_else_branch_of_negated_test_silences_declare_a() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] < 4)); then echo old; else declare -A m; fi\n");
}

#[test]
fn guard_early_exit_silences_later_statements() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] < 4)); then echo old >&2; exit 1; fi\ndeclare -A m\n");
}

#[test]
fn guard_early_return_silences_later_statements_in_function() {
    assert_silent("3.2", "f() {\n  if ((BASH_VERSINFO[0] < 4)); then return 1; fi\n  declare -A m\n}\n");
}

#[test]
fn guard_elif_body_is_guarded_by_its_own_condition() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] < 4)); then echo old\nelif ((BASH_VERSINFO[0] >= 4)); then declare -A m\nfi\n");
}

#[test]
fn guard_and_chain_right_side_is_guarded() {
    assert_silent("3.2", "((BASH_VERSINFO[0] >= 4)) && x=1 && declare -A m\n");
}

#[test]
fn guard_or_right_side_runs_only_when_guard_is_false() {
    assert_silent("3.2", "((BASH_VERSINFO[0] < 4)) || declare -A m\n");
}

#[test]
fn guard_or_exit_silences_later_statements() {
    assert_silent("3.2", "((BASH_VERSINFO[0] >= 4)) || exit 1\ndeclare -A m\n");
}

#[test]
fn guard_and_braced_exit_silences_later_statements() {
    assert_silent("3.2", "((BASH_VERSINFO[0] < 4)) && { echo old >&2; exit 1; }\ndeclare -A m\n");
}

#[test]
fn guard_negated_arith_silences_declare_a() {
    assert_silent("3.2", "if ! ((BASH_VERSINFO[0] < 4)); then declare -A m; fi\n");
}

#[test]
fn guard_arith_with_bare_name_is_element_zero() {
    assert_silent("3.2", "if ((BASH_VERSINFO >= 4)); then declare -A m; fi\n");
}

#[test]
fn guard_arith_with_dollar_name_is_element_zero() {
    assert_silent("3.2", "if (( $BASH_VERSINFO >= 4 )); then declare -A m; fi\n");
}

#[test]
fn guard_arith_with_braced_element_zero() {
    assert_silent("3.2", "if (( ${BASH_VERSINFO[0]} >= 4 )); then declare -A m; fi\n");
}

#[test]
fn guard_arith_greater_than_three_means_four() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] > 3)); then declare -A m; fi\n");
}

#[test]
fn guard_arith_equal_four_means_four() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] == 4)); then declare -A m; fi\n");
}

#[test]
fn guard_arith_literal_on_the_left_is_mirrored() {
    assert_silent("3.2", "if ((4 <= BASH_VERSINFO[0])); then declare -A m; fi\n");
    assert_silent("3.2", "if ((3 < BASH_VERSINFO[0])); then declare -A m; fi\n");
    assert_silent("3.2", "if ((4 == BASH_VERSINFO[0])); then declare -A m; fi\n");
}

#[test]
fn guard_arith_less_than_on_the_left_is_negated() {
    assert_silent("3.2", "if ((4 > BASH_VERSINFO[0])); then echo old; else declare -A m; fi\n");
}

#[test]
fn guard_arith_less_or_equal_three_means_below_four() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] <= 3)); then echo old; else declare -A m; fi\n");
}

#[test]
fn guard_double_bracket_numeric_operators() {
    assert_silent("3.2", "[[ ${BASH_VERSINFO[0]} -ge 4 ]] && declare -A m\n");
    assert_silent("3.2", "[[ $BASH_VERSINFO -gt 3 ]] && declare -A m\n");
    assert_silent("3.2", "[[ ${BASH_VERSINFO[0]} -eq 4 ]] && declare -A m\n");
    assert_silent("3.2", "[[ ${BASH_VERSINFO[0]} == 4 ]] && declare -A m\n");
}

#[test]
fn guard_double_bracket_bare_element_zero() {
    assert_silent("3.2", "[[ BASH_VERSINFO[0] -ge 4 ]] && declare -A m\n");
}

#[test]
fn guard_double_bracket_version_string_first_character() {
    assert_silent("3.2", "[[ ${BASH_VERSION:0:1} -ge 4 ]] && declare -A m\n");
}

#[test]
fn guard_test_and_bracket_numeric_operators() {
    assert_silent("3.2", "[ $BASH_VERSINFO -ge 4 ] && declare -A m\n");
    assert_silent("3.2", "test \"${BASH_VERSINFO[0]}\" -ge 4 && declare -A m\n");
    assert_silent("3.2", "test \"${BASH_VERSINFO[0]}\" -eq 4 && declare -A m\n");
    assert_silent("3.2", "[ 4 -le \"${BASH_VERSINFO[0]}\" ] && declare -A m\n");
}

#[test]
fn guard_test_negated_with_bang() {
    assert_silent("3.2", "test ! \"${BASH_VERSINFO[0]}\" -lt 4 && declare -A m\n");
}

#[test]
fn guard_combined_major_and_minor_gives_four_three() {
    // `4.3` is the release the second disjunct proves, so the 4.3 feature is silent.
    assert_silent("3.2", "(( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3) )) && declare -n r=x\n");
}

#[test]
fn guard_combined_major_and_minor_double_bracket() {
    assert_silent(
        "3.2",
        "[[ ${BASH_VERSINFO[0]} -gt 4 || (${BASH_VERSINFO[0]} -eq 4 && ${BASH_VERSINFO[1]} -ge 3) ]] && declare -n r=x\n",
    );
}

#[test]
fn guard_combined_major_and_minor_still_warns_above_the_guard() {
    // The guard is 4.3, so a 4.4 feature is still too new at target 3.2.
    assert_eq!(
        bash("3.2", "(( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3) )) && echo ${v@Q}\n"),
        [9125]
    );
}

#[test]
fn guard_and_of_major_and_minor_gives_four_three() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3)); then declare -n r=x; fi\n");
}

#[test]
fn guard_and_with_an_unknown_side_still_proves_the_known_side() {
    assert_silent("3.2", "if (( BASH_VERSINFO[0] >= 4 && x )); then declare -A m; fi\n");
}

#[test]
fn guard_or_with_unknown_side_proves_nothing() {
    assert_eq!(bash("3.2", "if (( BASH_VERSINFO[0] >= 4 || x )); then declare -A m; fi\n"), [9102]);
}

#[test]
fn guard_or_of_two_majors_proves_the_smaller() {
    // `>= 4 || >= 3` holds on 3.x too, so 4.0 features still warn.
    assert_eq!(bash("3.2", "if (( BASH_VERSINFO[0] >= 4 || BASH_VERSINFO[0] >= 3 )); then declare -A m; fi\n"), [9102]);
}

#[test]
fn guard_nested_guards_take_the_maximum() {
    // Outer guard 4.0 and inner guard 5.0: `$EPOCHSECONDS` (5.0) is silent, `$SRANDOM` (5.1) warns.
    assert_silent(
        "3.2",
        "if ((BASH_VERSINFO[0] >= 4)); then\n  if ((BASH_VERSINFO[0] >= 5)); then echo $EPOCHSECONDS; fi\nfi\n",
    );
    assert_eq!(
        bash(
            "3.2",
            "if ((BASH_VERSINFO[0] >= 4)); then\n  if ((BASH_VERSINFO[0] >= 5)); then echo $SRANDOM; fi\nfi\n"
        ),
        [9132]
    );
}

#[test]
fn guard_function_defined_inside_guarded_region_inherits_the_guard() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] >= 4)); then\n  f() { declare -A m; }\nfi\n");
}

#[test]
fn guard_covers_a_case_arm_in_its_body() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] >= 4)); then case x in y) declare -A m;; esac; fi\n");
}

// --- Near misses: a guard does not reach everything -------------------------------------------

#[test]
fn near_miss_guard_for_four_does_not_silence_four_three_feature() {
    assert_eq!(
        bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then declare -n r=x; fi\n"),
        [9122],
        "declare -n needs 4.3, and a 4.0 guard only proves 4.0"
    );
}

#[test]
fn near_miss_guard_for_four_does_not_silence_five_feature() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then echo $SRANDOM; fi\n"), [9132]);
}

#[test]
fn near_miss_guard_for_five_does_not_silence_five_one_feature() {
    assert_silent("3.2", "if ((BASH_VERSINFO[0] >= 5)); then echo $EPOCHSECONDS; fi\n");
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 5)); then echo $SRANDOM; fi\n"), [9132]);
}

#[test]
fn near_miss_guard_for_four_three_does_not_silence_four_four_feature() {
    assert_eq!(
        bash("3.2", "if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3) )); then echo ${v@Q}; fi\n"),
        [9125]
    );
}

#[test]
fn near_miss_message_still_names_the_configured_target() {
    assert_eq!(
        messages("3.2", "if ((BASH_VERSINFO[0] >= 4)); then declare -n r=x; fi\n"),
        ["declare -n requires bash 4.3 (target is 3.2)."]
    );
}

#[test]
fn near_miss_code_after_if_guard_without_exit_still_warns() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then :; fi\ndeclare -A m\n"), [9102]);
}

#[test]
fn near_miss_early_exit_not_at_the_end_of_its_body_does_not_count() {
    assert_eq!(
        bash("3.2", "if ((BASH_VERSINFO[0] < 4)); then exit 1; echo old; fi\ndeclare -A m\n"),
        [9102]
    );
}

#[test]
fn near_miss_exit_in_a_subshell_does_not_end_the_script() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] < 4)); then (exit 1); fi\ndeclare -A m\n"), [9102]);
}

#[test]
fn near_miss_negated_guard_body_still_warns() {
    // The old-bash branch: `< 4` is true on 3.x, so `declare -A` there is too new.
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] < 4)); then declare -A m; fi\n"), [9102]);
    assert_eq!(bash("3.2", "((BASH_VERSINFO[0] < 4)) && declare -A m\n"), [9102]);
}

#[test]
fn near_miss_positive_guard_else_branch_still_warns() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then :; else declare -A m; fi\n"), [9102]);
}

#[test]
fn near_miss_and_or_mixup_positive_guard_or_runs_when_old() {
    assert_eq!(bash("3.2", "((BASH_VERSINFO[0] >= 4)) || declare -A m\n"), [9102]);
}

#[test]
fn near_miss_and_or_mixup_negated_guard_and_runs_when_new() {
    assert_eq!(bash("3.2", "((BASH_VERSINFO[0] < 4)) && echo old; declare -A m\n"), [9102]);
    assert_eq!(bash("3.2", "((BASH_VERSINFO[0] >= 4)) && :\ndeclare -A m\n"), [9102]);
}

#[test]
fn near_miss_early_exit_in_if_body_does_not_leak_out_of_it() {
    assert_eq!(
        bash(
            "3.2",
            "if true; then\n  if ((BASH_VERSINFO[0] < 4)); then exit 1; fi\nfi\ndeclare -A m\n"
        ),
        [9102]
    );
}

#[test]
fn near_miss_guard_in_one_function_does_not_leak_into_another() {
    assert_eq!(
        bash("3.2", "f() {\n  if ((BASH_VERSINFO[0] >= 4)); then :; fi\n  declare -A m\n}\ng() { declare -A n; }\n"),
        [9102, 9102]
    );
}

#[test]
fn near_miss_early_return_in_one_function_does_not_leak_into_another() {
    assert_eq!(
        bash("3.2", "f() {\n  if ((BASH_VERSINFO[0] < 4)); then return 1; fi\n  declare -A m\n}\ng() { declare -A n; }\n"),
        [9102]
    );
}

#[test]
fn near_miss_guard_before_a_function_does_not_cover_its_body() {
    // A plain `if` guard covers only its own body, not the statements or functions after it.
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then :; fi\nf() { declare -A m; }\n"), [9102]);
}

#[test]
fn near_miss_non_literal_comparison_is_not_a_guard() {
    assert_eq!(bash("3.2", "x=4\nif ((BASH_VERSINFO[0] >= x)); then declare -A m; fi\n"), [9102]);
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4 + 0)); then declare -A m; fi\n"), [9102]);
}

#[test]
fn near_miss_guard_through_a_variable_is_not_recognised() {
    assert_eq!(
        bash("3.2", "v=${BASH_VERSINFO[0]}\nif ((v >= 4)); then declare -A m; fi\n"),
        [9102]
    );
}

#[test]
fn near_miss_case_on_version_string_is_not_recognised() {
    assert_eq!(bash("3.2", "case ${BASH_VERSION} in 4*|5*) declare -A m;; esac\n"), [9102]);
}

#[test]
fn near_miss_bare_name_in_single_bracket_is_a_string_not_a_version() {
    // `[ BASH_VERSINFO -ge 4 ]` compares the word "BASH_VERSINFO", which is not a number in bash.
    assert_eq!(bash("3.2", "if [ BASH_VERSINFO -ge 4 ]; then declare -A m; fi\n"), [9102]);
}

#[test]
fn near_miss_lexical_greater_than_in_double_bracket_is_not_numeric() {
    // `>` in `[[ ]]` compares strings, so `"10" > "4"` is false and it proves no major version.
    assert_eq!(bash("3.2", "[[ ${BASH_VERSINFO[0]} > 3 ]] && declare -A m\n"), [9102]);
}

#[test]
fn near_miss_unrecognised_operand_is_not_a_guard() {
    assert_eq!(bash("3.2", "if [ \"${BASH_VERSINFO[2]}\" -ge 4 ]; then declare -A m; fi\n"), [9102]);
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] >= 4)); then :; fi; if ((BASH_SUBSHELL >= 4)); then declare -A m; fi\n"), [9102]);
}

#[test]
fn near_miss_guard_arith_operand_with_expression_index_is_not_a_guard() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[1+0] >= 4)); then declare -A m; fi\n"), [9102]);
}

#[test]
fn near_miss_unsupported_operator_in_guard_is_not_a_guard() {
    assert_eq!(bash("3.2", "if ((BASH_VERSINFO[0] != 4)); then declare -A m; fi\n"), [9102]);
}

// --- Guards that name a version and the features on either side of it -------------------------

#[test]
fn guard_five_silences_five_zero_features_only() {
    // `declare -A` is 4.0, so the 5.0 guard silences it too. `$SRANDOM` is 5.1 and still warns.
    assert_eq!(
        bash("3.2", "if ((BASH_VERSINFO[0] >= 5)); then echo $EPOCHSECONDS; echo $SRANDOM; declare -A m; fi\n"),
        [9132]
    );
}

// --- Gating: no target, non-bash shells -------------------------------------------------------

#[test]
fn no_target_output_is_unchanged_by_guards() {
    assert_eq!(codes(Shell::Bash, None, "if ((BASH_VERSINFO[0] >= 4)); then declare -A m; fi\n"), Vec::<Code>::new());
}

#[test]
fn non_bash_shells_are_unchanged_by_guards() {
    for shell in [Shell::Sh, Shell::Dash, Shell::Ksh] {
        assert_eq!(
            codes(shell, Some("3.2"), "if ((BASH_VERSINFO[0] >= 4)); then declare -A m; fi\nif ((BASH_VERSINFO[0] < 4)); then declare -A n; fi\n"),
            Vec::<Code>::new(),
            "{shell:?} is not checked for bash versions"
        );
    }
}

#[test]
fn guard_condition_itself_never_warns() {
    // Every guard form above, with nothing guarded. None of them may warn at target 3.2.
    assert_silent(
        "3.2",
        "((BASH_VERSINFO[0] >= 4))\n[[ ${BASH_VERSION%%.*} -ge 4 ]]\n[ \"${BASH_VERSINFO[0]}\" -ge 4 ]\n(( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3) ))\n",
    );
}
