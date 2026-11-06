// SPDX-License-Identifier: GPL-3.0-or-later
//! Facade behaviour that does not need the CLI binary.

mod common;

use std::sync::Arc;

use shellcheck::{Linter, Severity, Shell, apply_fixes, list_checks};

fn codes(src: &str, linter: &Linter) -> Vec<u32> {
    linter.check_str(src).iter().map(|d| d.code).collect()
}

#[test]
fn linter_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Linter>();
}

#[test]
fn one_linter_shared_across_threads() {
    let linter = Arc::new(Linter::builder().shell(Shell::Bash).build());
    let src = "echo $1\n";
    let expected = linter.check_str(src);
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let linter = Arc::clone(&linter);
            std::thread::spawn(move || linter.check_str(src))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), expected);
    }
}

#[test]
fn exclude_and_include_filter_codes() {
    let src = "echo $1\n";
    let all = codes(src, &Linter::new());
    assert!(all.contains(&2086), "fixture should trigger SC2086, got {all:?}");

    let excluded = codes(src, &Linter::builder().exclude([2086]).build());
    assert!(!excluded.contains(&2086));

    let included = codes(src, &Linter::builder().include([2086]).build());
    assert!(included.contains(&2086));
    assert!(included.iter().all(|c| *c == 2086), "include should keep only SC2086, got {included:?}");
}

#[test]
fn severity_drops_less_serious_diagnostics() {
    let src = "echo $1\n";
    let warnings = Linter::builder().severity(Severity::Warning).build().check_str(src);
    assert!(warnings.iter().all(|d| d.level <= Severity::Warning));
    let everything = Linter::new().check_str(src);
    assert!(everything.len() > warnings.len());
}

#[test]
fn diagnostics_have_positions_and_fixes() {
    let src = "#!/bin/sh\nrm $file\n";
    let diags = Linter::builder().shell(Shell::Sh).build().check_str(src);
    let d = diags.iter().find(|d| d.code == 2086).expect("SC2086");
    assert_eq!((d.line, d.column), (2, 4));
    assert_eq!(d.end_line, 2);
    let fix = d.fix.as_ref().expect("SC2086 has a fix");
    // Positions refer to the whole checked script, so apply to that script.
    assert_eq!(fix.apply(src), "#!/bin/sh\nrm \"$file\"\n");
}

#[test]
fn fix_on_last_line_adds_newline_like_diff() {
    // Changing the last line gives a newline, as `-f diff` does.
    let src = "rm $file";
    let diags = Linter::builder().shell(Shell::Sh).build().check_str(src);
    let fix = diags.iter().find_map(|d| d.fix.as_ref()).expect("a fix");
    assert_eq!(fix.apply(src), "rm \"$file\"\n");
}

#[test]
fn untouched_last_line_keeps_missing_newline() {
    // The fix is on line 1, so line 2 (no newline) is unchanged.
    let src = "rm $file\necho done";
    let diags = Linter::builder().shell(Shell::Sh).build().check_str(src);
    let fix = diags.iter().find_map(|d| d.fix.as_ref()).expect("a fix");
    assert_eq!(fix.apply(src), "rm \"$file\"\necho done");
}

#[test]
fn apply_fixes_without_fixes_is_identity() {
    let src = "#!/bin/sh\necho hello\n";
    let diags = Linter::builder().shell(Shell::Sh).build().check_str(src);
    assert!(diags.is_empty());
    assert_eq!(apply_fixes(src, &diags), src);
}

#[test]
fn tabs_count_as_one_column() {
    // The tab counts as one column, so `$1` is at column 7 (t=1, echo=2-5, space=6).
    let diags = Linter::builder().shell(Shell::Bash).build().check_str("#!/bin/bash\n\techo $1\n");
    let d = diags.iter().find(|d| d.code == 2086).expect("SC2086");
    assert_eq!(d.column, 7);
}

#[test]
fn check_file_reads_latin1_fallback() {
    let diags = Linter::builder().shell(Shell::Sh).build().check_file(common::fixture("latin1.sh")).unwrap();
    assert!(diags.iter().any(|d| d.code == 2086));
}

#[test]
fn check_file_reports_missing_file() {
    let err = Linter::new().check_file("/nonexistent/definitely/missing.sh").unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn list_checks_is_sorted_and_unique() {
    let checks = list_checks();
    assert!(!checks.is_empty());
    let names: Vec<&str> = checks.iter().map(|c| c.name).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(names, sorted);
    assert!(checks.iter().all(|c| c.optional && !c.codes.is_empty()));
}
