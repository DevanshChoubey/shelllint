// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser (lane P-C): what the
// new zsh readers produce, beyond "it parses". Each snippet was accepted by
// `zsh -n` (zsh 5.9); every `zsh -fc` command quoted below was run as written.

#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

fn no_sc1xxx(script: &str) {
    let got = zsh_codes(script);
    let bad: Vec<u32> = got.iter().copied().filter(|c| (1000..2000).contains(c)).collect();
    assert!(bad.is_empty(), "SC1xxx {bad:?} for:\n{script}");
}

/// The conformance record `prop_readScript7`: with zsh picked explicitly the
/// whole script parses and reports no SC1xxx. (SC2086 on `echo $f` is the
/// checks lane's job, so only SC1xxx is asserted.)
#[test]
fn prop_read_script7_script_has_no_sc1xxx_under_shell_zsh() {
    no_sc1xxx("#!/bin/zsh\n# shellcheck disable=SC1071\nfor f (a b); echo $f\n");
}

/// Control for the test above: with the shebang present and `--shell=zsh`
/// the body really is analysed (an unassigned variable after the loop is
/// reported), so "no SC1xxx" is not the shebang shim swallowing the script.
#[test]
fn prop_read_script7_control_the_body_is_linted() {
    let got = zsh_codes("#!/bin/zsh\n# shellcheck disable=SC1071\nfor f (a b); echo $f\necho \"$undefined_var\"\n");
    assert!(got.contains(&2154), "{got:?}");
    assert!(!got.iter().any(|c| (1000..2000).contains(c)), "{got:?}");
}

/// `if . ./lib.sh; then` is read twice under zsh (the short-`if` attempt, then
/// the normal one); with `-x` the sourced file must still be read once and
/// give what bash gives.
#[test]
fn sourcing_inside_an_if_condition_is_the_same_under_zsh_and_bash() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("zsh_pc_source_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lib.sh"), "x=1\nunused_in_lib=2\n").unwrap();
    std::fs::write(dir.join("main.sh"), "if . ./lib.sh; then echo \"$x\"; fi\nif source ./lib.sh\nthen echo \"$x\"; fi\n").unwrap();
    let run = |shell: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
            .args(["--norc", "-x", "-f", "json1", &format!("--shell={shell}"), "main.sh"])
            .current_dir(&dir)
            .env_remove("SHELLCHECK_OPTS")
            .output()
            .unwrap();
        assert!(matches!(o.status.code(), Some(0) | Some(1)), "{}", String::from_utf8_lossy(&o.stderr));
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        v["comments"].as_array().unwrap().iter().map(|c| (c["code"].as_u64().unwrap(), c["file"].as_str().unwrap().to_string(), c["line"].as_u64().unwrap())).collect::<Vec<_>>()
    };
    let (zsh, bash) = (run("zsh"), run("bash"));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(zsh, bash);
}

#[test]
fn short_for_loop_body_is_part_of_the_loop() {
    // `$f` is the loop variable, so it is neither unassigned nor unused.
    assert_eq!(zsh_codes("for f (a b); echo \"$f\"\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("for f (a b) echo \"$f\"\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("for f in a b\necho \"$f\"\n"), Vec::<u32>::new());
}

#[test]
fn every_name_of_a_multi_variable_for_is_assigned() {
    // zsh -fc 'for k v in a 1 b 2; do echo $k=$v; done' prints a=1 and b=2.
    for script in [
        "for k v in a 1 b 2; do echo \"$k=$v\"; done\n",
        "for k v w in a 1 2 b 3 4; do echo \"$k$v$w\"; done\n",
        "for k v (a 1 b 2) echo \"$k=$v\"\n",
    ] {
        assert_eq!(zsh_codes(script), Vec::<u32>::new(), "{script}");
    }
}

#[test]
fn ordinary_for_loops_are_unchanged_under_zsh() {
    for script in [
        "for f in a b; do echo \"$f\"; done\n",
        "for f; do echo \"$f\"; done\n",
        "for ((i=0;i<3;i++)); do echo \"$i\"; done\n",
        "for f in a b; do\n  echo \"$f\"\ndone\n",
    ] {
        assert_eq!(zsh_codes(script), bash_codes(script), "{script}");
    }
}

#[test]
fn a_for_loop_without_do_in_bash_is_still_an_error() {
    // The short body is zsh only (`zsh -n` accepts it, `bash -n` does not).
    assert!(bash_codes("for f in a b; echo $f\n").contains(&1058));
    assert!(bash_codes("for f (a b) echo $f\n").contains(&1073));
}

#[test]
fn math_function_name_is_not_a_variable_read() {
    // zsh -fc 'zmodload zsh/mathfunc; x=-3; echo $(( abs(x) ))' prints 3.
    assert_eq!(zsh_codes("x=-3\n(( abs(x) == 3 )) && echo yes\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("(( rand48() < 2 )) && echo yes\n"), Vec::<u32>::new());
}

#[test]
fn math_function_arguments_are_still_analysed() {
    // `y` is read inside the call and was never assigned.
    assert!(zsh_codes("(( abs(y) == 3 )) && echo yes\n").contains(&2154));
}

#[test]
fn arithmetic_base_prefix_keeps_its_operand() {
    // zsh -fc 'echo $(( [##16] 255 ))' prints FF; the operand `n` is read.
    assert!(zsh_codes("echo $(( [##16] n ))\n").contains(&2154));
    assert_eq!(zsh_codes("n=255\necho $(( [##16] n ))\n"), Vec::<u32>::new());
}

#[test]
fn forced_append_redirects_to_the_named_file() {
    // zsh -fc 'echo a >>| f; echo b >>! f; echo c >! g' writes a, b to f and c to g.
    assert_eq!(zsh_codes("echo a >>| f\necho b >>! f\necho c >! g\n"), Vec::<u32>::new());
    // The target is the real file `f` (so SC2094 fires as for `>>` and `>|`),
    // not a stray `!` or `|` word.
    assert_eq!(zsh_codes("cat f >>| f\n"), bash_codes("cat f >> f\n"));
    assert_eq!(zsh_codes("cat f >>! f\n"), bash_codes("cat f >> f\n"));
    assert_eq!(zsh_codes("cat f >! f\n"), bash_codes("cat f >| f\n"));
}

#[test]
fn numeric_fd_variable_is_a_redirection_not_a_literal_brace() {
    for script in ["exec {1}<&-\n", "builtin exec {1}<&-\n", "exec {10}>file\n"] {
        let got = zsh_codes(script);
        assert!(!got.contains(&1083), "{script}: {got:?}");
    }
}

#[test]
fn prompt_escapes_do_not_report_sc1083_under_zsh() {
    for script in [
        "PS1=%{x%}\n",
        "PS1=%F{blue}x%f\n",
        "PROMPT=%{%F{blue}%}x\n",
        "git log @{u}..\n",
        "git rev-parse HEAD@{-1}\n",
        "PS1=%K{red}x%k\n",
        "PS1=%D{\"%H:%M\"}\n",
        "RPROMPT=%{$fg[red]%}x%{$reset_color%}\n",
    ] {
        let got = zsh_codes(script);
        assert!(!got.contains(&1083), "{script}: {got:?}");
        // ... and bash still reports them.
        assert!(bash_codes(script).contains(&1083), "bash lost SC1083 for {script}");
    }
}

#[test]
fn if_without_then_gives_the_same_nodes_checks_expect() {
    // The gallois shape: the condition list runs up to the closing `fi`.
    let script = "if true; then echo a\nelse\n  if ! [[ -z $x ]] && printf '%s' \"$x\"\n  local s=1\n  printf '%s' \"$s\"\nfi\n";
    no_sc1xxx(script);
    // a regular if is untouched
    assert_eq!(zsh_codes("if true; then echo a; fi\n"), bash_codes("if true; then echo a; fi\n"));
}
