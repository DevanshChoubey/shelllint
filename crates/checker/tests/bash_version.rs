// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `bash-version` (an extension beyond upstream): the spec field, the rc key and the
// directive, their precedence, and invalid values. The per-feature rows are in
// crates/checks/tests/bash_version_*.rs.

use shelllint_checker::check_script;
use shelllint_core::interface::{
    BashVersion, CheckSpec, Code, empty_check_spec, mock_rc_file, mocked_system_interface,
};

fn codes(rc: Option<&str>, spec: CheckSpec) -> Vec<Code> {
    let sys = match rc {
        Some(rc) => mock_rc_file(rc, mocked_system_interface(vec![])),
        None => mocked_system_interface(vec![]),
    };
    check_script(&sys, &spec).comments.into_iter().map(|c| c.comment.code).collect()
}

fn spec(script: &str) -> CheckSpec {
    CheckSpec { script: script.into(), ..empty_check_spec() }
}

fn target(v: &str) -> Option<BashVersion> {
    Some(v.parse().unwrap())
}

const PIPE: &str = "#!/bin/bash\ndate |& tee out\n";

#[test]
fn off_without_a_target() {
    assert_eq!(codes(None, spec(PIPE)), Vec::<Code>::new());
}

#[test]
fn spec_field() {
    let s = CheckSpec { bash_version: target("3.2"), ..spec(PIPE) };
    assert_eq!(codes(None, s), vec![9104]);
    let s = CheckSpec { bash_version: target("4.0"), ..spec(PIPE) };
    assert_eq!(codes(None, s), Vec::<Code>::new());
}

#[test]
fn rc_key() {
    assert_eq!(codes(Some("bash-version=3.2"), spec(PIPE)), vec![9104]);
    assert_eq!(codes(Some("bash-version=4.4"), spec(PIPE)), Vec::<Code>::new());
    assert_eq!(codes(Some("bash-version=\"3.2\""), spec(PIPE)), vec![9104]);
    assert_eq!(codes(Some("bash-version=4"), spec(PIPE)), Vec::<Code>::new());
}

#[test]
fn directive() {
    assert_eq!(codes(None, spec("#!/bin/bash\n# shellcheck bash-version=3.2\ndate |& tee out\n")), vec![9104]);
    assert_eq!(codes(None, spec("#!/bin/bash\n# shellcheck bash-version=4.0\ndate |& tee out\n")), Vec::<Code>::new());
    assert_eq!(codes(None, spec("#!/bin/bash\n# shellcheck bash-version='3.2'\ndate |& tee out\n")), vec![9104]);
    // It covers the whole file, not just the next command.
    let two = "#!/bin/bash\n# shellcheck bash-version=3.2\ntrue\n\nf() {\n  date |& tee out\n}\n";
    assert_eq!(codes(None, spec(two)), vec![9104]);
}

#[test]
fn bare_major_means_x_dot_zero() {
    assert_eq!(codes(Some("bash-version=4"), spec(PIPE)), Vec::<Code>::new());
    assert_eq!(codes(None, spec("#!/bin/bash\n# shellcheck bash-version=3\ndate |& tee out\n")), vec![9104]);
    assert_eq!(codes(None, spec("#!/bin/bash\n# shellcheck bash-version=4\ndate |& tee out\n")), Vec::<Code>::new());
}

#[test]
fn precedence_flag_over_directive_over_rc() {
    // The directive beats the rc key.
    let dir = "#!/bin/bash\n# shellcheck bash-version=4.0\ndate |& tee out\n";
    assert_eq!(codes(Some("bash-version=3.2"), spec(dir)), Vec::<Code>::new());
    let dir = "#!/bin/bash\n# shellcheck bash-version=3.2\ndate |& tee out\n";
    assert_eq!(codes(Some("bash-version=5.0"), spec(dir)), vec![9104]);
    // The flag beats both.
    let s = CheckSpec { bash_version: target("5.0"), ..spec(dir) };
    assert_eq!(codes(Some("bash-version=3.0"), s), Vec::<Code>::new());
    let s = CheckSpec {
        bash_version: target("3.2"),
        ..spec("#!/bin/bash\n# shellcheck bash-version=5.0\ndate |& tee out\n")
    };
    assert_eq!(codes(Some("bash-version=5.0"), s), vec![9104]);
}

#[test]
fn bad_directive_value_is_sc1146() {
    for bad in ["3.7", "2.5", "5.4", "6", "x", "3.2.57", "03.2", "4.", ".2"] {
        let c = codes(None, spec(&format!("#!/bin/bash\n# shellcheck bash-version={bad}\ndate |& tee out\n")));
        assert_eq!(c, vec![1146], "{bad}");
    }
}

#[test]
fn bad_rc_value_is_ignored() {
    // No target is set, and the file is judged as it is for any other bad rc value.
    let like_others = codes(Some("strict-env-vars=bogus"), spec(PIPE));
    for bad in ["9.9", "junk", "3.7", "3.2.1"] {
        let c = codes(Some(&format!("bash-version={bad}")), spec(PIPE));
        assert_eq!(c, like_others, "{bad}");
        assert!(!c.contains(&9104), "{bad}");
    }
}

#[test]
fn ignored_for_other_shells() {
    for shell in ["sh", "dash", "ksh"] {
        let s = CheckSpec { bash_version: target("3.0"), ..spec(&format!("#!/bin/{shell}\ndate |& tee out\n")) };
        assert!(!codes(None, s).contains(&9104), "{shell}");
    }
    assert!(!codes(Some("bash-version=3.0\nshell=sh"), spec("date |& tee out\n")).contains(&9104));
}

#[test]
fn can_be_excluded() {
    let s = CheckSpec { bash_version: target("3.2"), excluded_warnings: vec![9104], ..spec(PIPE) };
    assert_eq!(codes(None, s), Vec::<Code>::new());
    let s = CheckSpec {
        bash_version: target("3.2"),
        ..spec("#!/bin/bash\n# shellcheck disable=SC9104\ndate |& tee out\n")
    };
    assert_eq!(codes(None, s), Vec::<Code>::new());
}

#[test]
fn is_a_warning_and_not_an_optional_check() {
    let s = CheckSpec { bash_version: target("3.2"), ..spec(PIPE) };
    let sys = mocked_system_interface(vec![]);
    let r = check_script(&sys, &s);
    assert_eq!(r.comments.len(), 1);
    assert_eq!(r.comments[0].comment.severity, shelllint_core::interface::Severity::WarningC);
    assert!(shelllint_checker::optional_checks().iter().all(|d| !d.id.iter().any(|c| (9100..=9199).contains(c))));
}

#[test]
fn version_parsing() {
    for (s, v) in [("3.0", (3, 0)), ("3", (3, 0)), ("3.2", (3, 2)), ("4", (4, 0)), ("4.4", (4, 4)), ("5.3", (5, 3))] {
        let b: BashVersion = s.parse().unwrap();
        assert_eq!((b.major(), b.minor()), v, "{s}");
    }
    for s in ["", "2.9", "3.3", "4.5", "5.4", "6", "3.2.1", "3.", ".2", "03.2", "3.02", "a", "3,2", " 3.2", "3.2 "] {
        assert!(s.parse::<BashVersion>().is_err(), "{s:?}");
    }
    assert_eq!(BashVersion::new(4, 4).unwrap().to_string(), "4.4");
    assert!(BashVersion::new(3, 3).is_none());
    assert!(target("3.2") < target("4.0"));
    assert!(target("4.4") < target("5.0"));
}
