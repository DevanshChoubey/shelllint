// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the codes this lane turns off or rewrites under zsh
// (SC2277, SC2270, SC2275, SC2051, SC2180, SC2082, SC2296-2301, SC2338, SC2118).
//
// Each (d) code has a repro that is silent under zsh and unchanged under bash, with the
// zsh evidence next to it.
#[path = "zsh_msg_common.rs"]
mod zsh_msg_common;
use zsh_msg_common::*;

// ---- SC2277 / SC2270: positional and $0 assignment ----

#[test]
fn sc2277_bash_text_and_autofix_unchanged() {
    let f = bash("0=foo\n");
    assert_eq!(lane(&f), vec![2277]);
    assert_eq!(message_of(&f, 2277), "Use BASH_ARGV0 to assign to $0 in bash (or use [ ] to compare).");
    assert!(f[0].has_fix);
}

#[test]
fn sc2277_zsh_is_silent_and_never_offers_the_bash_argv0_fix() {
    let f = zsh("0=foo\necho \"$0\"\n");
    assert_eq!(lane(&f), Vec::<u32>::new());
    assert!(f.iter().all(|x| !x.has_fix));
    // evidence: `0=foo` is a plain assignment in zsh
    assert_eq!(zsh_run("0=foo; print $0"), "foo");
    // evidence: the bash autofix would break zsh (BASH_ARGV0 is not special; $0 is unchanged)
    assert!(!zsh_run("BASH_ARGV0=foo; print -r -- ${0##*/}").contains("foo"));
}

#[test]
fn sc2277_zsh_silent_inside_function_and_with_arguments() {
    assert_eq!(lane(&zsh("f() {\n  0=bar\n}\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("0=foo ls\n")), Vec::<u32>::new());
}

#[test]
fn sc2270_zsh_literal_positional_assignment_is_silent_bash_unchanged() {
    assert_eq!(lane(&bash("2=x\n")), vec![2270]);
    assert_eq!(
        message_of(&bash("2=x\n"), 2270),
        "To assign positional parameters, use 'set -- first second ..' (or use [ ] to compare)."
    );
    assert_eq!(lane(&zsh("2=x\n")), Vec::<u32>::new());
    assert_eq!(lane(&zsh("12=x\n")), Vec::<u32>::new());
    // evidence
    assert_eq!(zsh_run("set -- a b; 2=x; print $@"), "a x");
}

#[test]
fn sc2277_sc2270_expansion_forms_are_a_command_name_under_zsh() {
    // `${0}=42` and `$2=2` do not assign in zsh: they run a command named `...=42`.
    // That is the existing SC2276 meaning, so zsh gets it instead of the BASH_ARGV0 / set -- advice.
    let z = zsh("${0}=42\n");
    assert_eq!(lane(&z), vec![2276]);
    assert_eq!(lane(&bash("${0}=42\n")), vec![2277]);
    assert!(z.iter().all(|x| !x.has_fix));
    let z = zsh("$2=2\n");
    assert_eq!(lane(&z), vec![2276]);
    assert_eq!(lane(&bash("$2=2\n")), vec![2270]);
    // evidence
    assert!(zsh_run("set -- a b; $2=2").contains("command not found: b=2"));
}

#[test]
fn sc2277_sc2270_non_assignments_other_codes_unchanged_under_zsh() {
    // `$var=1` (SC2281) is still a bug in zsh
    assert_eq!(lane(&zsh("var=x\n$var=1\n")), lane(&bash("var=x\n$var=1\n")));
    assert!(zsh_run("a=1; $a=2").contains("command not found: 1=2"));
    assert_eq!(lane(&zsh("9foo=42\n")), lane(&bash("9foo=42\n")));
}

#[test]
fn sc2275_zsh_equals_expansion_command_is_valid() {
    // `=ls` is zsh's EQUALS expansion (path of ls)
    assert_eq!(lane(&bash("=ls /\n")), vec![2275]);
    assert_eq!(lane(&zsh("=ls /\n")), Vec::<u32>::new());
    assert!(!zsh_run("=ls / | head -1").is_empty() && !zsh_run("=ls / | head -1").contains("not found"));
}

#[test]
fn sc2275_zsh_still_reports_runs_of_equals() {
    // `=====x` is still not a command in zsh ("not found"), and conflict markers still apply
    assert_eq!(lane(&zsh("=====x\n")), lane(&bash("=====x\n")));
    assert_eq!(lane(&zsh("=======\n")), lane(&bash("=======\n")));
    assert!(zsh_run("=====x").contains("not found"));
}

// ---- SC2051: variables in brace ranges ----

#[test]
fn sc2051_zsh_variable_ranges_work_so_it_is_off() {
    assert_eq!(lane(&bash("n=3\necho {1..$n}\n")), vec![2051]);
    assert_eq!(
        message_of(&bash("n=3\necho {1..$n}\n"), 2051),
        "Bash doesn't support variables in brace range expansions."
    );
    assert!(!lane(&zsh("n=3\necho {1..$n}\n")).contains(&2051));
    assert_eq!(zsh_run("n=3; print {1..$n}"), "1 2 3");
    assert_eq!(zsh_run("a=(x y z); print {1..$#a}"), "1 2 3");
}

#[test]
fn sc2051_zsh_silent_in_loops_and_with_start_variable() {
    assert!(!lane(&zsh("n=3\nfor i in {1..$n}; do echo $i; done\n")).contains(&2051));
    assert!(!lane(&zsh("n=1\nm=2\necho {$n..$m}\n")).contains(&2051));
    assert_eq!(zsh_run("n=1; m=3; print {$n..$m}"), "1 2 3");
}

// ---- SC2180: multidimensional arrays ----

#[test]
fn sc2180_zsh_is_off() {
    assert_eq!(lane(&bash("a[1][2]=x\n")), vec![2180]);
    assert_eq!(
        message_of(&bash("a[1][2]=x\n"), 2180),
        "Bash does not support multidimensional arrays. Use 1D or associative arrays."
    );
    assert!(!lane(&zsh("a[1][2]=x\n")).contains(&2180));
    assert!(!lane(&zsh("echo \"${a[1][2]}\"\n")).contains(&2180));
    // evidence: `${a[1][1]}` is a valid zsh expansion (a substring of element 1), and
    // `m[a,b]` is zsh's emulated multi-dimensional key
    assert_eq!(zsh_run("a=(12 2); print ${a[1][1]}"), "1");
    assert_eq!(zsh_run("typeset -A m; m[a,b]=1; print ${m[a,b]}"), "1");
}

// ---- SC2338 (optional) ----

#[test]
fn sc2338_zsh_is_off_even_when_enabled() {
    let s = "[[ $x = y ]]\n";
    assert!(lane(&lint(&["--shell=bash", "--enable=require-double-equals"], s)).contains(&2338));
    assert!(!lane(&lint(&["--shell=zsh", "--enable=require-double-equals"], s)).contains(&2338));
    // evidence: both spellings are valid zsh
    assert_eq!(zsh_run("x=y; [[ $x = y ]] && [[ $x == y ]] && print ok"), "ok");
}

// ---- SC2118 (ksh only) ----

#[test]
fn sc2118_stderr_pipe_stays_ksh_only() {
    assert_eq!(lane(&ksh("a |& b\n")), vec![2118]);
    assert!(!lane(&zsh("a |& b\n")).contains(&2118));
    zsh_parses("a |& b");
}

// ---- SC2082 and SC2296-2301: bad parameter substitution ----

const NESTED: [&str; 4] = ["echo ${$(echo x)}\n", "echo ${${x}}\n", "echo ${(f)x}\n", "echo ${${x}:-d}\n"];
const OFF_CODES: [u32; 7] = [2082, 2296, 2298, 2299, 2300, 2301, 2297];

#[test]
fn sc2082_family_zsh_silent_on_valid_zsh_expansions() {
    for s in NESTED {
        let z = lane(&zsh(s));
        assert!(z.iter().all(|c| !OFF_CODES.contains(c)), "{s:?} -> {z:?}");
    }
    // evidence: nested, command-substitution and flag forms are valid zsh
    assert_eq!(zsh_run("x=hi; print ${${x}}"), "hi");
    assert_eq!(zsh_run("print ${$(echo hey)}"), "hey");
    assert_eq!(zsh_run("x=$'a\\nb'; print ${#${(f)x}}"), "2");
    for s in NESTED {
        zsh_parses(s);
    }
}

#[test]
fn sc2082_family_bash_still_reports_next_to_zsh() {
    let b: Vec<u32> = NESTED.iter().flat_map(|s| lane(&bash(s))).collect();
    assert!(b.iter().any(|c| OFF_CODES.contains(c)), "bash reported nothing: {b:?}");
}

#[test]
fn sc2297_stays_under_zsh_because_zsh_rejects_the_form_too() {
    let z = zsh("echo ${\"x\"}\n");
    assert_eq!(lane(&z), vec![2297]);
    assert_eq!(lane(&bash("echo ${\"x\"}\n")), vec![2297]);
    assert!(zsh_run("print ${\"x\"}").contains("bad substitution"));
}
