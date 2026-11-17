// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the `--bash-version` syntax checks (SC9104 9105 9106 9109 9113 9137) and of the
// rules every SC91xx code shares; no Haskell counterpart.

mod bash_version_common;

use bash_version_common::*;
use shelllint_checks::shell_support::check_bash_version::FEATURES;
use shelllint_core::interface::Shell;

#[test]
fn pipe_and_sc9104() {
    assert_pair(9104, "4.0", "ls |& cat\n");
    assert_pair(9104, "4.0", "make 2>/dev/null |& tee log\n");
    // Every occurrence is reported.
    assert_eq!(bash("3.2", "a |& b\nc |& d\n"), [9104, 9104]);
}

#[test]
fn pipe_false_positives() {
    assert_silent("3.0", "ls | cat\n");
    assert_silent("3.0", "ls 2>&1 | cat\n");
    assert_silent("3.0", "a && b || c\n");
    assert_silent("3.0", "echo 'a |& b'\n");
}

#[test]
fn case_terminators_sc9105() {
    assert_pair(9105, "4.0", "case $x in a) echo a ;& b) echo b ;; esac\n");
    assert_pair(9105, "4.0", "case $x in a) echo a ;;& b) echo b ;; esac\n");
    // The message names the terminator that is used.
    assert_eq!(messages("3.2", "case $x in a) echo a ;& b) echo b ;; esac\n"), [";& requires bash 4.0 (target is 3.2)."]);
    assert_eq!(messages("3.2", "case $x in a) echo a ;;& b) echo b ;; esac\n"), [";;& requires bash 4.0 (target is 3.2)."]);
    // One finding per clause.
    assert_eq!(bash("3.2", "case $x in a) :;& b) :;& c) :;; esac\n"), [9105, 9105]);
}

#[test]
fn case_false_positives() {
    assert_silent("3.0", "case $x in a) echo a ;; b) echo b ;; esac\n");
    assert_silent("3.0", "case $x in a) echo a\n;;\nb) echo b\nesac\n");
    assert_silent("3.0", "case $x in a|b) echo ab ;; esac\n");
}

#[test]
fn coproc_sc9106() {
    assert_pair(9106, "4.0", "coproc cat\n");
    assert_pair(9106, "4.0", "coproc NAME { cat; }\n");
    assert_pair(9106, "4.0", "coproc sort -n\n");
}

#[test]
fn coproc_false_positives() {
    assert_silent("3.0", "echo coproc\n");
    assert_silent("3.0", "cat coproc.txt\n");
    assert_silent("3.0", "coproc=1; echo \"$coproc\"\n");
}

#[test]
fn append_both_sc9109() {
    assert_pair(9109, "4.0", "echo hi &>> log\n");
    assert_pair(9109, "4.0", "cmd &>>\"$f\"\n");
}

#[test]
fn append_both_false_positives() {
    assert_silent("3.0", "echo hi &> log\n");
    assert_silent("3.0", "echo hi >> log 2>&1\n");
    assert_silent("3.0", "echo hi >> log\n");
    assert_silent("3.0", "echo hi & echo there >> log\n");
}

#[test]
fn fd_allocation_sc9113() {
    assert_pair(9113, "4.1", "exec {fd}>file\n");
    assert_pair(9113, "4.1", "exec {fd}<file\n");
    assert_pair(9113, "4.1", "cat <&3 {out}>/dev/null\n");
    assert_pair(9113, "4.1", "exec {fd}>&-\n");
}

#[test]
fn fd_allocation_false_positives() {
    assert_silent("3.0", "exec 3>file\n");
    assert_silent("3.0", "exec 3>&-\n");
    assert_silent("3.0", "{ echo hi; } > file\n");
    assert_silent("3.0", "echo {a,b}\n");
    assert_silent("3.0", "echo {fd}\n");
    assert_silent("3.0", "echo ${fd}>file\n");
}

#[test]
fn command_substitution_sc9137() {
    assert_pair(9137, "5.3", "x=${ date; }\n");
    assert_pair(9137, "5.3", "x=${| REPLY=1; }\n");
    assert_pair(9137, "5.3", "echo \"${ date; }\"\n");
}

#[test]
fn command_substitution_false_positives() {
    assert_silent("3.0", "x=${y}\n");
    assert_silent("3.0", "x=$(date)\n");
    assert_silent("3.0", "x=${y:-$(date)}\n");
    assert_silent("3.0", "x=${#y}\n");
}

#[test]
fn message_names_feature_version_and_target() {
    assert_eq!(messages("3.2", "ls |& cat\n"), ["|& requires bash 4.0 (target is 3.2)."]);
    assert_eq!(messages("4.0", "exec {fd}>f\n"), ["{fd}> fd allocation requires bash 4.1 (target is 4.0)."]);
    assert_eq!(messages("5.2", "x=${ date; }\n"), ["${ cmd; } command substitution requires bash 5.3 (target is 5.2)."]);
}

#[test]
fn every_older_target_fires() {
    for t in ["3.0", "3.1", "3.2"] {
        assert_eq!(bash(t, "ls |& cat\n"), [9104], "{t}");
    }
    for t in ["4.0", "4.1", "4.2", "4.3", "4.4", "5.0", "5.1", "5.2", "5.3"] {
        assert_eq!(bash(t, "ls |& cat\n"), Vec::<i64>::new(), "{t}");
    }
}

#[test]
fn no_target_means_no_findings() {
    let all = "ls |& cat\ncase $x in a) :;& b) :;;& esac\ncoproc cat\necho >> f &>> f\nexec {fd}>f\ny=${ date; }\n";
    assert_eq!(codes(Shell::Bash, None, all), Vec::<i64>::new());
    assert!(!bash("3.0", all).is_empty());
}

#[test]
fn ignored_for_other_shells() {
    let script = "ls |& cat\nexec {fd}>f\n";
    for shell in [Shell::Sh, Shell::Dash, Shell::Ksh, Shell::BusyboxSh] {
        assert_eq!(codes(shell, Some("3.0"), script), Vec::<i64>::new(), "{shell:?}");
    }
    assert_eq!(bash("3.0", script), [9104, 9113]);
}

#[test]
fn table_is_complete_and_ordered() {
    assert_eq!(FEATURES.len(), 38);
    for (i, f) in FEATURES.iter().enumerate() {
        assert_eq!(f.code, 9100 + i as i64);
        assert!(!f.name.is_empty());
    }
    // Versions never go down as the code goes up.
    assert!(FEATURES.windows(2).all(|w| w[0].version <= w[1].version));
}
