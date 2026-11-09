// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the shelllint-only `--modern-dialects` option (rc key `modern-dialects=true`).

use shelllint_checker::check_script;
use shelllint_core::interface::{
    CheckSpec, Code, empty_check_spec, mock_rc_file, mocked_system_interface,
};

fn codes(script: &str, modern_flag: bool, rc: &str) -> Vec<Code> {
    let spec = CheckSpec {
        script: script.into(),
        included_warnings: Some(vec![1146, 2190, 2191, 3036, 3037, 3045]),
        optional_checks: if modern_flag { vec!["modern-dialects".to_string()] } else { vec![] },
        ..empty_check_spec()
    };
    let sys = mock_rc_file(rc, mocked_system_interface(vec![]));
    let mut v: Vec<Code> = check_script(&sys, &spec).comments.into_iter().map(|pc| pc.comment.code).collect();
    v.sort();
    v
}

const READ_D: &str = "#!/bin/sh\nread -d '' x\n";
const READ_T_ASH: &str = "#!/bin/ash\nread -t 5 x\n";
const READ_T_BUSYBOX: &str = "#!/bin/busybox sh\nread -t 5 x\n";
const ECHO_E_ASH: &str = "#!/bin/ash\necho -e 'a\\tb'\n";
const ECHO_E_SH_BUSYBOX: &str = "#!/usr/bin/env busybox\n# shellcheck shell=sh\necho -e 'a\\tb'\n";
const ASSOC: &str = "#!/bin/bash\ndeclare -A a\na=(k v k2 v2)\n";

#[test]
fn read_d_off_warns() {
    assert_eq!(codes(READ_D, false, ""), vec![3045]);
}

#[test]
fn read_d_on_is_clean() {
    assert_eq!(codes(READ_D, true, ""), vec![]);
    assert_eq!(codes(READ_D, false, "modern-dialects=true"), vec![]);
}

#[test]
fn read_d_on_still_warns_in_dash() {
    assert_eq!(codes("#!/bin/dash\nread -d '' x\n", true, ""), vec![3045]);
}

#[test]
fn read_t_ash_off_warns() {
    assert_eq!(codes(READ_T_ASH, false, ""), vec![3045]);
}

#[test]
fn read_t_ash_on_is_clean() {
    assert_eq!(codes(READ_T_ASH, true, ""), vec![]);
    assert_eq!(codes(READ_T_ASH, false, "modern-dialects=true"), vec![]);
}

#[test]
fn read_t_busybox_pair() {
    assert_eq!(codes(READ_T_BUSYBOX, false, ""), vec![3045]);
    assert_eq!(codes(READ_T_BUSYBOX, true, ""), vec![]);
}

#[test]
fn read_t_plain_sh_stays_warned() {
    assert_eq!(codes("#!/bin/sh\nread -t 5 x\n", true, ""), vec![3045]);
}

#[test]
fn echo_e_ash_pair() {
    assert_eq!(codes(ECHO_E_ASH, false, ""), vec![3036]);
    assert_eq!(codes(ECHO_E_ASH, true, ""), vec![]);
}

#[test]
fn echo_e_sh_with_busybox_shebang_pair() {
    assert_eq!(codes(ECHO_E_SH_BUSYBOX, false, ""), vec![3037]);
    assert_eq!(codes(ECHO_E_SH_BUSYBOX, true, ""), vec![]);
}

#[test]
fn echo_e_plain_sh_stays_warned() {
    assert_eq!(codes("#!/bin/sh\necho -e hi\n", true, ""), vec![3037]);
}

#[test]
fn assoc_pairs_off_warns() {
    assert_eq!(codes(ASSOC, false, ""), vec![2190, 2190, 2190, 2190]);
}

#[test]
fn assoc_pairs_on_is_clean() {
    assert_eq!(codes(ASSOC, true, ""), vec![]);
    assert_eq!(codes(ASSOC, false, "modern-dialects=true"), vec![]);
}

#[test]
fn assoc_odd_count_still_warns() {
    assert_eq!(codes("#!/bin/bash\ndeclare -A a\na=(k v k2)\n", true, ""), vec![2190, 2190, 2190]);
}

#[test]
fn rc_false_and_bad_value() {
    assert_eq!(codes(READ_D, false, "modern-dialects=false"), vec![3045]);
    assert_eq!(codes(READ_D, false, "modern-dialects=maybe"), vec![3045]);
}
