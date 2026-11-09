// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the shelllint-only disable-start / disable-end / disable-line directives.

use shelllint_checker::check_script_codes;
use shelllint_core::interface::{Code, Shell};

fn codes(script: &str) -> Vec<Code> {
    // Only the codes these tests are about; 1107/1073/... must never show up.
    check_script_codes(script, Some(Shell::Bash)).into_iter().filter(|c| !matches!(c, 2154 | 2250 | 2248)).collect()
}

#[test]
fn baseline_reports() {
    assert_eq!(codes("#!/bin/bash\necho $a\n"), vec![2086]);
}

#[test]
fn start_end_range() {
    let s = "#!/bin/bash\necho $a\n# shellcheck disable-start=SC2086\necho $b\necho $c\n# shellcheck disable-end=SC2086\necho $d\n";
    assert_eq!(codes(s), vec![2086, 2086]);
}

#[test]
fn multiple_codes_and_all() {
    let s =
        "#!/bin/bash\n# shellcheck disable-start=SC2086,SC2034\nx=1\necho $b\n# shellcheck disable-end=all\necho $d\n";
    assert_eq!(codes(s), vec![2086]);
    let s = "#!/bin/bash\n# shellcheck disable-start=all\nx=1\necho $b\n# shellcheck disable-end=all\necho $d\n";
    assert_eq!(codes(s), vec![2086]);
}

#[test]
fn unterminated_range_runs_to_eof() {
    let s = "#!/bin/bash\necho $a\n# shellcheck disable-start=SC2086\necho $b\necho $c\n";
    assert_eq!(codes(s), vec![2086]);
}

#[test]
fn trailing_same_line() {
    let s = "#!/bin/bash\necho $a # shellcheck disable-line=SC2086\necho $b\n";
    assert_eq!(codes(s), vec![2086]);
}

#[test]
fn other_codes_outside_range_still_report() {
    let s = "#!/bin/bash\n# shellcheck disable-start=SC2086\necho $b\nx=`date`\n# shellcheck disable-end=SC2086\nx=`date`\n";
    assert_eq!(codes(s), vec![2006, 2034, 2006]);
}

#[test]
fn end_for_other_code_keeps_range() {
    let s = "#!/bin/bash\n# shellcheck disable-start=SC2086\n# shellcheck disable-end=SC2034\necho $b\n";
    assert_eq!(codes(s), Vec::<Code>::new());
}

#[test]
fn valid_anywhere() {
    let s = "#!/bin/bash\nif true; then\n  # shellcheck disable-start=SC2086\n  echo $a\n  # shellcheck disable-end=SC2086\nfi\necho $b |\n  # shellcheck disable-line=SC2086\n  cat\n";
    assert_eq!(codes(s), vec![2086]);
}

#[test]
fn quoted_hash_is_not_a_directive() {
    let s = "#!/bin/bash\necho \"x # shellcheck disable-line=SC2086\" $a\n";
    assert_eq!(codes(s), vec![2086]);
}
