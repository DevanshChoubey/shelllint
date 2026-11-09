// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--ignore-directives` and `--report-unused-disables` (SC9006),
// extensions beyond upstream ShellCheck. Issues koalaman/shellcheck#1417, #645.

use shelllint_checker::{check_script, optional_checks};
use shelllint_core::interface::{CheckSpec, Severity, empty_check_spec, mock_rc_file, mocked_system_interface};

const UNUSED: i64 = 9006;

fn spec(script: &str) -> CheckSpec {
    CheckSpec { script: script.into(), ignore_rc: true, filename: "t.sh".into(), ..empty_check_spec() }
}

/// (line, code, message) of every comment.
fn run(spec: CheckSpec) -> Vec<(i64, i64, String)> {
    check_script(&mocked_system_interface(vec![]), &spec)
        .comments
        .into_iter()
        .map(|pc| (pc.start_pos.line, pc.comment.code, pc.comment.message))
        .collect()
}

fn codes(spec: CheckSpec) -> Vec<(i64, i64)> {
    run(spec).into_iter().map(|(l, c, _)| (l, c)).collect()
}

fn ignoring(script: &str) -> CheckSpec {
    CheckSpec { ignore_directives: true, ..spec(script) }
}

fn auditing(script: &str) -> CheckSpec {
    CheckSpec { report_unused_disables: true, ..spec(script) }
}

const FILE_LEVEL: &str = "#!/bin/sh\n# shellcheck disable=SC2086\necho $1\n";
const COMMAND_LEVEL: &str = "#!/bin/sh\necho hi\n# shellcheck disable=SC2086\necho $1\n";

#[test]
fn directives_are_honoured_by_default() {
    assert_eq!(codes(spec(FILE_LEVEL)), vec![]);
    assert_eq!(codes(spec(COMMAND_LEVEL)), vec![]);
}

#[test]
fn ignore_directives_reports_what_a_file_level_directive_hides() {
    assert_eq!(codes(ignoring(FILE_LEVEL)), vec![(3, 2086)]);
}

#[test]
fn ignore_directives_reports_what_a_command_level_directive_hides() {
    assert_eq!(codes(ignoring(COMMAND_LEVEL)), vec![(4, 2086)]);
}

#[test]
fn ignore_directives_covers_disable_all_and_ranges() {
    let script = "#!/bin/sh\n# shellcheck disable=all\necho $1\n";
    assert_eq!(codes(ignoring(script)), vec![(3, 2086)]);
    let script = "#!/bin/sh\n# shellcheck disable=SC2000-SC2100\necho $1\n";
    assert_eq!(codes(ignoring(script)), vec![(3, 2086)]);
}

#[test]
fn ignore_directives_keeps_other_directives() {
    // shell= still applies: with sh, `[[` is an error; with bash it is fine.
    let script = "# shellcheck shell=bash disable=SC2086\n[[ -n $1 ]] && echo $1\n";
    assert_eq!(codes(ignoring(script)), vec![(2, 2086)]);
}

#[test]
fn ignore_directives_also_unhides_parser_messages() {
    let script = "#!/bin/sh\n# shellcheck disable=SC1090\n. \"$1\"\n";
    assert_eq!(codes(spec(script)), vec![]);
    assert_eq!(codes(ignoring(script)), vec![(3, 1090)]);
}

#[test]
fn ignore_directives_leaves_rc_files_alone() {
    let sys = mock_rc_file("disable=SC2086\n", mocked_system_interface(vec![]));
    let mut s = ignoring(FILE_LEVEL);
    s.ignore_rc = false;
    let got: Vec<i64> = check_script(&sys, &s).comments.into_iter().map(|pc| pc.comment.code).collect();
    assert_eq!(got, Vec::<i64>::new());
}

#[test]
fn unused_disables_are_off_by_default() {
    assert_eq!(codes(spec("#!/bin/sh\n# shellcheck disable=SC2034\necho hi\n")), vec![]);
}

#[test]
fn used_directives_are_not_reported() {
    assert_eq!(codes(auditing(FILE_LEVEL)), vec![]);
    assert_eq!(codes(auditing(COMMAND_LEVEL)), vec![]);
    let parser_note = "#!/bin/sh\n# shellcheck disable=SC1090\n. \"$1\"\n";
    assert_eq!(codes(auditing(parser_note)), vec![]);
}

#[test]
fn only_the_unused_code_of_a_list_is_reported() {
    let got = run(auditing("#!/bin/sh\n# shellcheck disable=SC2034,SC2086\necho $1\n"));
    assert_eq!(got, vec![(2, UNUSED, "This disable directive suppresses nothing: SC2034".to_string())]);
}

#[test]
fn each_unused_code_is_reported_on_its_own_directive_line() {
    let script = "#!/bin/sh\n# shellcheck disable=SC2034\n# shellcheck disable=SC2155,SC2086\necho $1\n";
    let got = run(auditing(script));
    assert_eq!(
        got,
        vec![
            (2, UNUSED, "This disable directive suppresses nothing: SC2034".to_string()),
            (3, UNUSED, "This disable directive suppresses nothing: SC2155".to_string()),
        ]
    );
}

#[test]
fn report_has_info_severity_and_the_directive_position() {
    let sys = mocked_system_interface(vec![]);
    let r = check_script(&sys, &auditing("#!/bin/sh\n  # shellcheck disable=SC2034\necho hi\n"));
    assert_eq!(r.comments.len(), 1);
    let pc = &r.comments[0];
    assert_eq!(pc.comment.severity, Severity::InfoC);
    assert_eq!((pc.start_pos.line, pc.start_pos.column), (2, 3));
}

#[test]
fn a_command_level_directive_only_counts_its_own_command() {
    // The directive covers `echo hi`; SC2086 on the next line is not hidden.
    let script = "#!/bin/sh\necho start\n# shellcheck disable=SC2086\necho hi\necho $1\n";
    assert_eq!(
        run(auditing(script)),
        vec![
            (3, UNUSED, "This disable directive suppresses nothing: SC2086".to_string()),
            (5, 2086, "Double quote to prevent globbing and word splitting.".to_string()),
        ]
    );
}

#[test]
fn ranges_and_all_are_reported_with_their_text() {
    let got = run(auditing("#!/bin/sh\n# shellcheck disable=SC2003-SC2010\necho hi\n"));
    assert_eq!(got, vec![(2, UNUSED, "This disable directive suppresses nothing: SC2003-SC2010".to_string())]);
}

#[test]
fn a_range_that_hides_something_is_used() {
    assert_eq!(codes(auditing("#!/bin/sh\n# shellcheck disable=SC2000-SC2100\necho $1\n")), vec![]);
}

#[test]
fn disabled_optional_checks_do_not_make_a_directive_look_unused() {
    let optional = optional_checks().into_iter().find(|d| d.id.contains(&2248)).expect("SC2248 is optional");
    let script = "#!/bin/sh\n# shellcheck disable=SC2248\necho hi\n";
    // Not enabled: the check never ran, so nothing can be said.
    assert_eq!(codes(auditing(script)), vec![]);
    // Enabled and still nothing to hide: now it is unused.
    let enabled = CheckSpec { optional_checks: vec![optional.name.clone()], ..auditing(script) };
    assert_eq!(codes(enabled), vec![(2, UNUSED)]);
    // A range or `all` that contains a disabled optional check is not judged either.
    assert_eq!(codes(auditing("#!/bin/sh\n# shellcheck disable=all\necho hi\n")), vec![]);
    // Enabled by a directive in the file.
    let by_directive =
        format!("#!/bin/sh\n# shellcheck enable={}\n# shellcheck disable=SC2248\necho hi\n", optional.name);
    assert_eq!(codes(auditing(&by_directive)), vec![(3, UNUSED)]);
}

#[test]
fn an_optional_check_that_a_directive_does_hide_is_used() {
    let optional = optional_checks().into_iter().find(|d| d.id.contains(&2248)).expect("SC2248 is optional");
    let script = "#!/bin/sh\nvar=1\n# shellcheck disable=SC2248\necho $var\n";
    let enabled = CheckSpec { optional_checks: vec![optional.name.clone()], ..auditing(script) };
    assert_eq!(codes(enabled), vec![]);
}

#[test]
fn output_filters_do_not_make_a_directive_look_unused() {
    // Only SC9006 is included; the SC2086 the directive hides is not shown
    // either way, and the directive is still seen as used.
    let only_report = |script: &str| CheckSpec { included_warnings: Some(vec![UNUSED]), ..auditing(script) };
    assert_eq!(codes(only_report(FILE_LEVEL)), vec![]);
    assert_eq!(codes(only_report("#!/bin/sh\n# shellcheck disable=SC2034\necho hi\n")), vec![(2, UNUSED)]);
    // Excluding the code (-e SC2086) or raising the severity (-S error) is no different.
    let excluded = CheckSpec { excluded_warnings: vec![2086, 2148], ..auditing(FILE_LEVEL) };
    assert_eq!(codes(excluded), vec![]);
    let errors_only = CheckSpec { min_severity: Severity::ErrorC, ..auditing(FILE_LEVEL) };
    assert_eq!(codes(errors_only), vec![]);
}

#[test]
fn both_flags_together_show_everything_and_report_the_dead_directive() {
    let script = "#!/bin/sh\n# shellcheck disable=SC2034\necho $1\n";
    let both = CheckSpec { ignore_directives: true, report_unused_disables: true, ..spec(script) };
    assert_eq!(codes(both), vec![(2, UNUSED), (3, 2086)]);
}

#[test]
fn misplaced_directives_are_left_alone() {
    // A directive inside a here-document is plain text.
    let script = "#!/bin/sh\ncat <<EOF\n# shellcheck disable=SC2034\nEOF\n";
    assert_eq!(codes(auditing(script)), vec![]);
}
