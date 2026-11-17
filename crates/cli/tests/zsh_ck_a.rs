// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh work (no ShellCheck counterpart).
//
// Test layer 4, class (a): codes that are true in zsh exactly as in bash. One zsh repro
// each shows they still fire.
//
// zsh evidence: `cd /tmp` then `ls $(pwd)` is word-split-free in zsh but the command
// substitution is still unquoted; `rm -rf $dir/` still expands an empty variable to `/`.
#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

fn has(codes: &[u32], c: u32) -> bool {
    codes.contains(&c)
}

#[test]
fn sc2164_cd_without_exit_still_fires() {
    assert!(has(&zsh_codes("cd /tmp\nls\n"), 2164));
}

#[test]
fn sc2115_rm_rf_unset_var_still_fires() {
    assert!(has(&zsh_codes("rm -rf \"$dir/\"\n"), 2115));
}

#[test]
fn sc2006_backticks_still_fire() {
    assert!(has(&zsh_codes("x=`date`\necho \"$x\"\n"), 2006));
}

#[test]
fn sc2005_useless_echo_still_fires() {
    assert!(has(&zsh_codes("echo \"$(date)\"\n"), 2005));
}

#[test]
fn sc2010_ls_grep_still_fires() {
    assert!(has(&zsh_codes("ls | grep foo\n"), 2010));
}

#[test]
fn sc2043_single_item_loop_still_fires() {
    assert!(has(&zsh_codes("for x in \"a b c\"; do echo \"$x\"; done\n"), 2043));
}

#[test]
fn sc2050_constant_test_still_fires() {
    assert!(has(&zsh_codes("[[ a = b ]]\n"), 2050));
}

#[test]
fn sc2046_unquoted_command_substitution_still_fires() {
    assert!(has(&zsh_codes("ls $(pwd)\n"), 2046));
}

#[test]
fn sc2126_grep_wc_still_fires() {
    assert!(has(&zsh_codes("grep foo f | wc -l\n"), 2126));
}

#[test]
fn sc2034_unused_assignment_still_fires() {
    assert!(has(&zsh_codes("x=1\n"), 2034));
}

#[test]
fn sc2154_unassigned_reference_still_fires() {
    assert!(has(&zsh_codes("echo \"$never_set\"\n"), 2154));
}
