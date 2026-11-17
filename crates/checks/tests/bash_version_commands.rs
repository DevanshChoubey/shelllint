// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the `--bash-version` command checks (SC9100 9102 9103 9111 9112 9114 9115 9116 9119 9120 9122 9124
// 9126 9127 9128 9130 9131 9133 9136); no Haskell counterpart.

mod bash_version_common;

use bash_version_common::*;

#[test]
fn printf_v_sc9100() {
    assert_pair(9100, "3.1", "printf -v x '%s' a\n");
    assert_eq!(messages("3.0", "printf -v x '%s' a\n"), ["printf -v requires bash 3.1 (target is 3.0)."]);
    assert_silent("3.0", "printf '%s\\n' a\n");
    assert_silent("3.0", "printf -- '-v %s' a\n");
    assert_silent("3.0", "echo printf -v x\n");
}

#[test]
fn printf_v_array_sc9115() {
    assert_pair(9115, "4.1", "printf -v 'a[1]' '%s' a\n");
    assert_pair(9115, "4.1", "printf -v \"a[$i]\" '%s' a\n");
    assert_eq!(bash("3.0", "printf -v 'a[1]' '%s' a\n"), [9100, 9115]);
    assert_silent("3.0", "printf '%s' 'a[1]'\n");
    assert_eq!(bash("3.0", "printf -v x '%s' 'a[1]'\n"), [9100]);
}

#[test]
fn declare_a_sc9102() {
    assert_pair(9102, "4.0", "declare -A m\n");
    assert_pair(9102, "4.0", "typeset -A m\n");
    assert_pair(9102, "4.0", "f() { local -A m; }\n");
    assert_pair(9102, "4.0", "declare -rA m\n");
    assert_eq!(messages("3.2", "local -A m\n"), ["local -A requires bash 4.0 (target is 3.2)."]);
    assert_silent("3.0", "declare -a m\n");
    assert_silent("3.0", "declare -r m=1\n");
    assert_silent("3.0", "declare -x -i m=1\n");
    assert_silent("3.0", "declare m -A\n");
    assert_silent("3.0", "echo declare -A\n");
}

#[test]
fn mapfile_sc9103() {
    assert_pair(9103, "4.0", "mapfile -t lines < f\n");
    assert_pair(9103, "4.0", "readarray -t lines < f\n");
    assert_silent("3.0", "while read -r l; do echo \"$l\"; done < f\n");
    assert_silent("3.0", "echo mapfile\n");
}

#[test]
fn mapfile_d_sc9127() {
    assert_pair(9127, "4.4", "mapfile -d '' -t a < f\n");
    assert_pair(9127, "4.4", "mapfile -td '' a < f\n");
    assert_eq!(bash("3.2", "readarray -d , a < f\n"), [9103, 9127]);
    assert_silent("3.0", "echo mapfile -d\n");
    // -d only counts as an option of mapfile, not as the argument of another one.
    assert_eq!(bash("3.2", "mapfile -n 2 a < f\n"), [9103]);
    assert_eq!(bash("3.2", "mapfile -O -d a < f\n"), [9103]);
}

#[test]
fn shopt_globstar_sc9111() {
    assert_pair(9111, "4.0", "shopt -s globstar\n");
    assert_pair(9111, "4.0", "shopt -s nullglob globstar\n");
    assert_silent("3.0", "shopt -u globstar\n");
    assert_silent("3.0", "shopt -q globstar\n");
    assert_silent("3.0", "shopt globstar\n");
    assert_silent("3.0", "shopt -s nullglob\n");
    assert_silent("3.0", "shopt -s extglob\n");
}

#[test]
fn shopt_lastpipe_sc9119() {
    assert_pair(9119, "4.2", "shopt -s lastpipe\n");
    assert_silent("3.0", "shopt -u lastpipe\n");
    assert_silent("3.0", "shopt -q lastpipe\n");
    assert_silent("3.0", "set -o pipefail\n");
}

#[test]
fn shopt_inherit_errexit_sc9128() {
    assert_pair(9128, "4.4", "shopt -s inherit_errexit\n");
    assert_silent("3.0", "shopt -u inherit_errexit\n");
    assert_silent("3.0", "shopt -q inherit_errexit\n");
    assert_silent("3.0", "set -e\n");
}

#[test]
fn shopt_localvar_sc9131() {
    assert_pair(9131, "5.0", "shopt -s localvar_inherit\n");
    assert_pair(9131, "5.0", "shopt -s localvar_unset\n");
    assert_eq!(bash("4.4", "shopt -s localvar_inherit localvar_unset\n"), [9131, 9131]);
    assert_silent("3.0", "shopt -u localvar_inherit\n");
    assert_silent("3.0", "shopt -q localvar_unset\n");
}

#[test]
fn shopt_patsub_sc9136() {
    assert_pair(9136, "5.2", "shopt -s patsub_replacement\n");
    assert_silent("3.0", "shopt -u patsub_replacement\n");
    assert_silent("3.0", "shopt -q patsub_replacement\n");
    assert_silent("3.0", "shopt -s nocasematch\n");
}

#[test]
fn read_t_fractional_sc9112() {
    assert_pair(9112, "4.0", "read -t 0.5 x\n");
    assert_pair(9112, "4.0", "read -r -t 2.5 x\n");
    assert_pair(9112, "4.0", "read -t.5 x\n");
    assert_silent("3.0", "read -t 5 x\n");
    assert_silent("3.0", "read -t5 x\n");
    assert_silent("3.0", "read -t \"$timeout\" x\n");
    assert_silent("3.0", "read -r -p '0.5 ' x\n");
}

#[test]
fn read_n_upper_sc9114() {
    assert_pair(9114, "4.1", "read -N 3 x\n");
    assert_pair(9114, "4.1", "read -rN3 x\n");
    assert_silent("3.0", "read -n 3 x\n");
    assert_silent("3.0", "read -r x\n");
    assert_silent("3.0", "read -p N x\n");
}

#[test]
fn declare_g_sc9116() {
    assert_pair(9116, "4.2", "declare -g x=1\n");
    assert_pair(9116, "4.2", "typeset -g x=1\n");
    assert_eq!(messages("4.1", "declare -g x=1\n"), ["declare -g requires bash 4.2 (target is 4.1)."]);
    assert_silent("3.0", "declare -x x=1\n");
    assert_silent("3.0", "declare x=-g\n");
}

#[test]
fn printf_time_sc9120() {
    assert_pair(9120, "4.2", "printf '%(%Y-%m-%d)T\\n' -1\n");
    assert_pair(9120, "4.2", "printf -v d '%(%s)T' -1\n");
    assert_silent("3.0", "printf '%s\\n' '%(%Y)T'\n");
    assert_silent("3.0", "printf '100%%(x)\\n'\n");
    assert_silent("3.0", "printf '%d\\n' 3\n");
}

#[test]
fn declare_n_sc9122() {
    assert_pair(9122, "4.3", "declare -n ref=var\n");
    assert_pair(9122, "4.3", "f() { local -n ref=$1; }\n");
    assert_pair(9122, "4.3", "typeset -n ref=var\n");
    assert_eq!(messages("4.2", "local -n r=v\n"), ["local -n requires bash 4.3 (target is 4.2)."]);
    assert_silent("3.0", "declare -r ref=var\n");
    assert_silent("3.0", "declare -a arr\n");
    assert_silent("3.0", "export -n ref\n");
}

#[test]
fn wait_flags() {
    assert_pair(9124, "4.3", "wait -n\n");
    assert_pair(9130, "5.0", "wait -f $pid\n");
    assert_pair(9133, "5.1", "wait -n -p done\n");
    assert_silent("3.0", "wait $pid\n");
    assert_silent("3.0", "wait\n");
    assert_silent("3.0", "wait %1\n");
}

#[test]
fn wait_p_with_n() {
    assert_eq!(bash("4.2", "wait -n -p job\n"), [9124, 9133]);
    // The variable name after -p is not read as flags.
    assert_eq!(bash("3.0", "wait -p n\n"), [9133]);
}

#[test]
fn local_dash_sc9126() {
    assert_pair(9126, "4.4", "f() { local -; set +x; }\n");
    assert_pair(9126, "4.4", "f() { local - x; }\n");
    assert_silent("3.0", "f() { local x=-; }\n");
    assert_silent("3.0", "f() { local -r x=1; }\n");
    assert_silent("3.0", "declare -\n");
    assert_silent("3.0", "echo local -\n");
}

#[test]
fn command_prefix_and_other_shells() {
    use shelllint_core::interface::{Code, Shell};
    assert_eq!(bash("3.2", "command declare -A m\n"), [9102]);
    assert_eq!(bash("3.2", "builtin mapfile -t a\n"), [9103]);
    assert_eq!(codes(Shell::Sh, Some("3.2"), "declare -A m\nmapfile a\n"), Vec::<Code>::new());
    assert_eq!(codes(Shell::Bash, None, "declare -A m\nmapfile a\n"), Vec::<Code>::new());
}
