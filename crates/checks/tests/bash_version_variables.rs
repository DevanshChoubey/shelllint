// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the `--bash-version` variable rows: SC9101 9110 9118 9121 9123 9129 9132.

mod bash_version_common;
use bash_version_common::*;

#[test]
fn sc9101_append() {
    assert_pair(9101, "3.1", "x=a\nx+=b\n");
    assert_pair(9101, "3.1", "a=(1)\na+=(2)\n");
}

#[test]
fn sc9101_declare_append() {
    assert_pair(9101, "3.1", "declare x+=y\n");
}

#[test]
fn sc9101_guards() {
    assert_silent("3.0", "x=a\nx=\"$x b\"\ndeclare -a y\nlet n+=1\n(( n += 1 ))\n");
}

#[test]
fn sc9110_bashpid() {
    assert_pair(9110, "4.0", "echo $BASHPID\n");
    assert_pair(9110, "4.0", "echo \"${BASHPID}\"\n");
    assert_pair(9110, "4.0", "echo $(( BASHPID + 1 ))\n");
}

#[test]
fn sc9110_guards() {
    assert_silent("3.0", "echo $x_BASHPID $BASHPIDX ${BASHPIDX}\nBASHPID=1\n");
    assert_silent("3.0", "(( BASHPID = 1 ))\n");
}

#[test]
fn sc9118_v_test() {
    assert_pair(9118, "4.2", "[[ -v x ]]\n");
    assert_pair(9118, "4.2", "[ -v x ]\n");
    assert_pair(9118, "4.2", "test -v x\n");
    assert_pair(9118, "4.2", "[[ ! -v x ]]\n");
}

#[test]
fn sc9118_guards() {
    assert_silent("3.0", "[[ -n x ]]\n[ -z x ]\ntest -e x\n[[ x == -v ]]\ntest -v\n");
}

#[test]
fn sc9121_negative_index_assign() {
    assert_pair(9121, "4.3", "a[-1]=x\n");
    assert_pair(9121, "4.3", "a[-2]+=x\n");
    assert_pair(9121, "4.3", "unset 'a[-1]'\n");
    assert_pair(9121, "4.3", "unset a[-1]\n");
}

#[test]
fn sc9121_guards() {
    assert_silent("3.1", "a[1]=x\na[i-1]=x\na[$i]=x\na[-i]=x\nunset 'a[1]'\nunset a[i]\nunset -v a\n");
}

#[test]
fn sc9123_nameref_test() {
    assert_pair(9123, "4.3", "[[ -R x ]]\n");
    assert_pair(9123, "4.3", "test -R x\n");
}

#[test]
fn sc9123_guards() {
    assert_silent("3.0", "[[ -r x ]]\n[ -r x ]\nR=1\n");
}

#[test]
fn sc9129_epoch_and_argv0() {
    assert_pair(9129, "5.0", "echo $EPOCHSECONDS\n");
    assert_pair(9129, "5.0", "echo ${EPOCHREALTIME}\n");
    assert_pair(9129, "5.0", "echo $BASH_ARGV0\n");
    assert_pair(9129, "5.0", "echo $(( EPOCHSECONDS - 1 ))\n");
}

#[test]
fn sc9129_message_names_variable() {
    assert_eq!(messages("4.4", "echo $EPOCHSECONDS\n"), ["$EPOCHSECONDS requires bash 5.0 (target is 4.4)."]);
}

#[test]
fn sc9129_guards() {
    assert_silent("3.0", "echo $x_EPOCHSECONDS $EPOCHSECONDSX $BASH_ARGV01\nBASH_ARGV0=x\nEPOCHSECONDS=1\n");
}

#[test]
fn sc9132_srandom() {
    assert_pair(9132, "5.1", "echo $SRANDOM\n");
    assert_pair(9132, "5.1", "echo ${SRANDOM}\n");
    assert_pair(9132, "5.1", "echo $(( SRANDOM % 6 ))\n");
}

#[test]
fn sc9132_guards() {
    assert_silent("3.0", "echo $x_SRANDOM $SRANDOMX ${SRANDOMX}\nSRANDOM=1\n");
}

#[test]
fn every_occurrence_is_reported() {
    assert_eq!(bash("3.2", "echo $BASHPID $BASHPID\n"), [9110, 9110]);
}

#[test]
fn off_without_target_or_for_other_shells() {
    use shelllint_core::interface::Shell;
    let s = "x+=1\necho $BASHPID $SRANDOM\n[[ -v x ]]\n";
    assert_eq!(codes(Shell::Bash, None, s), Vec::<i64>::new());
    assert_eq!(codes(Shell::Sh, Some("3.0"), s), Vec::<i64>::new());
}
