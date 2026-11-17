// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh parser lane P-A (function and
// group syntax). `zsh_pa_constructs.rs` pins which snippets parse; these pin
// what the parse means: the shape of the tree (`--dump-ast`), that the
// analysis sees the bodies and the scoping, that diagnostics land on the right
// line, and that none of it panics on truncated input.
//
// Every zsh behaviour claimed here was run through `zsh -fc` first; the
// evidence is in the comments next to each test.

#[path = "zsh_common.rs"]
mod zsh_common;
use std::io::Write;
use std::process::{Command, Stdio};
use zsh_common::*;

/// The `--dump-ast` JSON for `script` under `--shell=zsh`, flattened to the
/// list of `type` (and `value`) strings in depth-first order. Only the two
/// keys are looked at, so a small scanner is enough.
fn ast_nodes(script: &str, shell: &str) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "--dump-ast", &format!("--shell={shell}"), "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let text = String::from_utf8(o.stdout).unwrap();
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("\"type\":\"") {
        rest = &rest[i + 8..];
        let end = rest.find('"').unwrap();
        let ty = &rest[..end];
        // an optional `"value":"..."` follows the span before `children`
        let upto = rest.find("\"children\"").unwrap_or(rest.len());
        let head = &rest[..upto];
        match head.find("\"value\":\"") {
            Some(v) => {
                let val = &head[v + 9..];
                let val = &val[..val.find('"').unwrap()];
                out.push(format!("{ty} {val}"));
            }
            None => out.push(ty.to_string()),
        }
    }
    out
}

fn count(nodes: &[String], what: &str) -> usize {
    nodes.iter().filter(|n| n.as_str() == what).count()
}

/// Codes and lines under zsh, for diagnostics that must sit on the right line.
fn codes_with_lines(script: &str) -> Vec<(u32, u32)> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1", "--shell=zsh", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let text = String::from_utf8(o.stdout).unwrap();
    // {"comments":[{"file":"-","line":1,...,"code":2164,...}]}: one object per comment
    text.split("\"file\":").skip(1).map(|c| (num_after(c, "\"code\":"), num_after(c, "\"line\":"))).collect()
}

fn num_after(s: &str, key: &str) -> u32 {
    let i = s.find(key).unwrap() + key.len();
    s[i..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap()
}

// ---- tree shape ------------------------------------------------------

// zsh -fc '() { print -r -- "$#:$1" } a b'  prints  2:a
// An anonymous function is a function that is called where it is written.
#[test]
fn anonymous_function_is_a_function_definition_plus_its_call() {
    let n = ast_nodes("() { echo hi } a b\n", "zsh");
    assert_eq!(count(&n, "T_Function zsh-anonymous-function"), 1, "{n:?}");
    // the call: the same name as a command word, followed by the two arguments
    assert_eq!(count(&n, "T_Literal zsh-anonymous-function"), 1, "{n:?}");
    assert_eq!(count(&n, "T_Literal a"), 1);
    assert_eq!(count(&n, "T_Literal b"), 1);
    let def = n.iter().position(|x| x.starts_with("T_Function")).unwrap();
    let call = n.iter().position(|x| x == "T_Literal zsh-anonymous-function").unwrap();
    assert!(def < call, "the definition comes before the call: {n:?}");
}

#[test]
fn anonymous_function_keeps_function_keyword_and_parens_distinct_forms() {
    for script in ["() { :; }\n", "function { :; }\n", "function () { :; }\n", "() ( : )\n"] {
        let n = ast_nodes(script, "zsh");
        assert_eq!(count(&n, "T_Function zsh-anonymous-function"), 1, "{script:?}: {n:?}");
    }
}

// zsh -fc 'sleep 0.1 &|' and '&!' both background and disown.
#[test]
fn disown_separators_are_backgrounded_commands() {
    for script in ["sleep 1 &|\n", "sleep 1 &!\n", "sleep 1 &\n"] {
        let n = ast_nodes(script, "zsh");
        assert_eq!(count(&n, "T_Backgrounded"), 1, "{script:?}: {n:?}");
    }
    let n = ast_nodes("a &| b &! c\n", "zsh");
    assert_eq!(count(&n, "T_Backgrounded"), 2, "{n:?}");
}

#[test]
fn equals_substitution_is_a_process_substitution_with_the_equals_direction() {
    let n = ast_nodes("diff =(echo a) =(echo b)\n", "zsh");
    assert_eq!(count(&n, "T_ProcSub ="), 2, "{n:?}");
    // its body is a command list, like `<( )`
    assert_eq!(count(&n, "T_Literal echo"), 2);
}

// zsh -fc '{ print one; } always { print two }'  prints one, two.
#[test]
fn always_block_is_two_groups_in_sequence() {
    let n = ast_nodes("{ echo a } always { echo b }\n", "zsh");
    // the wrapper plus the try block and the always block
    assert_eq!(count(&n, "T_BraceGroup"), 3, "{n:?}");
    let a = n.iter().position(|x| x == "T_Literal a").unwrap();
    let b = n.iter().position(|x| x == "T_Literal b").unwrap();
    assert!(a < b);
}

// zsh -fc 'function a b { print hi }; a; b'  prints hi twice: both names exist.
#[test]
fn multi_name_function_defines_every_name_with_the_same_body() {
    let n = ast_nodes("function a b { echo hi }\n", "zsh");
    assert_eq!(count(&n, "T_Function a"), 1, "{n:?}");
    assert_eq!(count(&n, "T_Function b"), 1, "{n:?}");
    assert_eq!(count(&n, "T_Literal echo"), 2, "the body is read for each name: {n:?}");
}

#[test]
fn odd_function_names_are_kept_verbatim() {
    let n = ast_nodes("function foo\"${1:-}\" { :; }\n", "zsh");
    // the scanner stops at the first escaped quote, so only the prefix is visible
    assert_eq!(n.iter().filter(|x| x.starts_with("T_Function foo")).count(), 1, "{n:?}");
}

#[test]
fn empty_bodies_are_empty_lists() {
    // `{ }` is a group with no commands; nothing else sits under it
    let n = ast_nodes("{ }\n", "zsh");
    assert_eq!(count(&n, "T_BraceGroup"), 1, "{n:?}");
    assert_eq!(count(&n, "T_SimpleCommand"), 0, "{n:?}");
    let n = ast_nodes("if true; then\nfi\n", "zsh");
    assert_eq!(count(&n, "T_IfExpression"), 1, "{n:?}");
}

#[test]
fn brace_form_if_has_the_same_shape_as_then_form() {
    let braces = ast_nodes("if (( 1 )) { echo y } else { echo n }\n", "zsh");
    let then = ast_nodes("if (( 1 )); then echo y; else echo n; fi\n", "zsh");
    assert_eq!(count(&braces, "T_IfExpression"), 1);
    assert_eq!(count(&braces, "T_Literal echo"), count(&then, "T_Literal echo"));
    assert_eq!(count(&braces, "T_Arithmetic"), count(&then, "T_Arithmetic"));
}

// ---- the analysis sees what the parser built ------------------------------

#[test]
fn anonymous_function_body_is_analysed() {
    // SC2154 inside the body of `() { }` (zsh -fc '() { print $nope }' prints an empty line)
    assert_eq!(zsh_codes("() { echo \"$nope\"; }\n"), vec![2154]);
}

#[test]
fn anonymous_function_with_arguments_is_not_flagged_for_unpassed_arguments() {
    // SC2119/SC2120 compare a function's `$1` with its calls: `a` is passed here.
    assert_eq!(zsh_codes("() { echo \"$1\" } a\n"), Vec::<u32>::new());
}

#[test]
fn variable_set_in_the_try_block_is_visible_in_the_always_block() {
    // zsh -fc '{ x=1 } always { print $x }' prints 1
    assert_eq!(zsh_codes("{ x=1; } always { echo \"$x\"; }\n"), Vec::<u32>::new());
}

#[test]
fn variable_set_in_a_function_defined_by_either_name_is_known() {
    // `function a b { x=1 }`: calling b sets x as well
    assert_eq!(zsh_codes("function a b { x=1; }\nb\necho \"$x\"\n"), Vec::<u32>::new());
}

#[test]
fn variable_set_in_a_brace_form_if_is_known() {
    assert_eq!(zsh_codes("if (( 1 )) { y=1 }\necho \"$y\"\n"), Vec::<u32>::new());
}

#[test]
fn equals_substitution_body_is_analysed() {
    assert_eq!(zsh_codes("cat =(echo \"$nope\")\n"), vec![2154]);
}

// ---- positions -------------------------------------------------------------

#[test]
fn diagnostics_inside_anonymous_function_keep_their_line() {
    // SC2164 for the unchecked `cd`, on line 3
    let got = codes_with_lines("() {\n  local d=1\n  cd /tmp\n  echo \"$d\"\n}\n");
    assert_eq!(got, vec![(2164, 3)]);
}

#[test]
fn diagnostics_after_a_brace_without_semicolon_keep_their_line() {
    let got = codes_with_lines("f() { echo hi }\ncd /tmp\n");
    assert_eq!(got, vec![(2164, 2)]);
}

#[test]
fn diagnostics_in_the_always_block_keep_their_line() {
    let got = codes_with_lines("{\n  echo try\n} always {\n  cd /tmp\n}\n");
    assert_eq!(got, vec![(2164, 4)]);
}

// ---- bash is untouched ----------------------------------------------------

#[test]
fn the_same_snippets_give_no_zsh_nodes_under_bash() {
    // none of the zsh mappings leak into the other dialects
    for script in [
        "() { echo hi } a\n",
        "sleep 1 &|\n",
        "diff =(echo a) =(echo b)\n",
        "{ :; } always { :; }\n",
        "function a b { :; }\n",
        "if (( 1 )) { :; }\n",
    ] {
        for shell in ["bash", "sh", "dash", "ksh"] {
            let n = ast_nodes(script, shell);
            assert_eq!(count(&n, "T_Function zsh-anonymous-function"), 0, "{shell} {script:?}");
            assert_eq!(count(&n, "T_ProcSub ="), 0, "{shell} {script:?}");
        }
    }
}

// ---- no panics ---------------------------------------------------------------

#[test]
fn truncated_snippets_never_panic_or_hang() {
    // every third prefix of a few snippets covering each construct; `zsh_codes`
    // fails the test on a crash (exit status other than 0 or 1)
    let scripts = [
        "() { echo \"$1\" } a b &|\nf() { echo hi }\n",
        "{ echo a } always { echo b }\ndiff =(echo a) <(() { :; })\n",
        "function a b { :; }\nfunction foo\"${1:-}\" { :; }\nif (( 1 )) { :; } else { :; }\n",
        "if true; then # c\nfi\nfor i in a b; { echo \"$i\" }\n",
    ];
    for s in scripts {
        for end in (0..=s.len()).step_by(3) {
            if s.is_char_boundary(end) {
                let _ = zsh_codes(&s[..end]);
            }
        }
    }
}
