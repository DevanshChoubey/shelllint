// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests (zsh is an extension).
//
// Integration-fix tests: each script below was checked against `zsh -n` (or
// `zsh -fc`) when it was written; `zsh_agrees` re-checks it when zsh is installed.

use std::process::Command;

fn codes(script: &str) -> Vec<String> {
    let d = std::env::temp_dir().join(format!("shelllint-zshint-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let f = d.join(format!("t{n}.zsh"));
    std::fs::write(&f, script).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-s", "zsh", "-f", "json1"])
        .arg(&f)
        .env_remove("SHELLCHECK_OPTS")
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let mut v: Vec<String> = out
        .split("\"code\":")
        .skip(1)
        .map(|s| format!("SC{}", s.chars().take_while(char::is_ascii_digit).collect::<String>()))
        .collect();
    v.sort();
    v
}

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `Some(true)` when `zsh -n` accepts the script, `Some(false)` when it reports a
/// parse error, `None` when zsh is not installed.
fn zsh_accepts(script: &str) -> Option<bool> {
    let o = Command::new("zsh").args(["-fnc", script]).output().ok()?;
    Some(o.status.success())
}

fn zsh_agrees(script: &str, ok: bool) {
    if let Some(got) = zsh_accepts(script) {
        assert_eq!(got, ok, "zsh -n disagrees about {script:?}");
    }
}

fn parses(script: &str) {
    zsh_agrees(script, true);
    let c = codes(script);
    for parse_code in ["SC1088", "SC1089", "SC1072", "SC1073", "SC1009"] {
        assert!(!c.contains(&parse_code.to_string()), "{script:?}: {c:?}");
    }
}

// ---- `}` handling --------------------------------------------------------

#[test]
fn brace_inside_a_word_is_literal() {
    parses("echo a}b\n");
    parses("f() { echo a}b }; f\n");
}

#[test]
fn brace_ending_a_word_closes_the_group() {
    parses("{ echo a}\n");
}

#[test]
fn brace_at_word_start_closes_each_group() {
    parses("{ { echo hi }}\n");
}

#[test]
fn stray_top_level_brace_is_a_parse_error() {
    zsh_agrees("echo x}\n", false);
    assert!(codes("echo x}\n").contains(&"SC1089".to_string()));
}

// ---- short if -------------------------------------------------------------

#[test]
fn short_if_one_line() {
    parses("if [[ -z $1 ]] echo hi\n");
    parses("if [[ -z $1 ]] { echo hi }\n");
}

#[test]
fn short_if_multi_line_else_branch() {
    parses("if [[ -n $1 ]]; then\n  :\nelse\n  if ! [[ -z $1 ]] && printf x\n  local s=1\n  echo $s\nfi\n");
}

#[test]
fn short_if_with_trailing_fi_is_rejected_like_zsh() {
    zsh_agrees("if [[ -z $1 ]] echo hi; fi\n", false);
    assert!(codes("if [[ -z $1 ]] echo hi; fi\n").contains(&"SC1089".to_string()));
}

// ---- SC1087 follows KSH_ARRAYS -------------------------------------------

#[test]
fn sc1087_is_silent_by_default_and_fires_with_ksharrays() {
    assert!(!codes("a=(x)\necho $a[1]\n").contains(&"SC1087".to_string()));
    assert!(codes("setopt ksharrays\na=(x)\necho $a[1]\n").contains(&"SC1087".to_string()));
    assert!(!codes("setopt ksharrays\na=(x)\necho ${a[1]}\n").contains(&"SC1087".to_string()));
}

// ---- shebang ---------------------------------------------------------------

#[test]
fn zsh_shebang_body_is_linted() {
    let d = std::env::temp_dir().join(format!("shelllint-zshint-sb-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let f = d.join("s");
    std::fs::write(&f, "#!/usr/bin/env zsh\nx=$(date)\necho $x\n").unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint")).args(["--norc"]).arg(&f).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).is_empty(), "{}", String::from_utf8_lossy(&o.stdout));
    std::fs::write(&f, "#!/usr/bin/env zsh\nread \"r?go\"\necho $y\n").unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint")).args(["--norc"]).arg(&f).output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).contains("SC2154"));
}

// ---- case terminators, disown separators, empty bodies, repeat -------------

#[test]
fn case_continue_testing_terminator() {
    let s = "case $1 in a) echo 1 ;| b) echo 2 ;& c) : ;; esac\n";
    parses(s);
    assert!(!codes(s).contains(&"SC2127".to_string()));
}

#[test]
fn disown_separators() {
    parses("sleep 1 &|; echo a\n");
    parses("sleep 1 &!; echo a\n");
    parses("sleep 1 &; echo a\n");
}

#[test]
fn empty_do_group() {
    parses("for i in 1 2; do done\n");
    parses("while false; do done\n");
}

#[test]
fn repeat_loop() {
    parses("repeat 3 echo hi\n");
    parses("repeat 3; do echo hi; done\n");
    parses("repeat 2 a=(1 2)\n");
}

// ---- subscripts and double-quoted braces ------------------------------------

#[test]
fn brace_in_double_quoted_default_does_not_nest() {
    parses("echo \"${x:-{a}\"\n");
}

#[test]
fn subscript_flag_needs_balanced_parens() {
    parses("typeset -A m; echo ${m[(r)a]}\n");
}

#[test]
fn associative_array_index_flags() {
    parses("typeset -A m\nm[(e)a b]=1\n");
}

// ---- rulings ------------------------------------------------------------------

#[test]
fn sc2086_still_fires_in_single_bracket_and_test() {
    // zsh drops an empty unquoted word: `x=; [ $x = y ]` is `[ = y ]`.
    if let Ok(o) = Command::new("zsh").args(["-fc", "x=; [ $x = y ]"]).output() {
        assert!(String::from_utf8_lossy(&o.stderr).contains("condition expected"));
    }
    assert!(codes("x=$1\n[ $x = y ]\n").contains(&"SC2086".to_string()));
    assert!(codes("test $1 = y\n").contains(&"SC2086".to_string()));
    assert!(codes("[ $1 = y ]\n").contains(&"SC2086".to_string()));
}

#[test]
fn sc2086_stays_quiet_in_double_bracket_and_plain_commands() {
    assert!(!codes("[[ $1 = y ]]\n").contains(&"SC2086".to_string()));
    assert!(!codes("echo $1\n").contains(&"SC2086".to_string()));
    assert!(!codes("[ \"$1\" = y ]\n").contains(&"SC2086".to_string()));
}

#[test]
fn sc2119_sc2120_skip_anonymous_functions() {
    let c = codes("() { echo $1; }\n() { echo $1; } a\n");
    assert!(!c.contains(&"SC2119".to_string()) && !c.contains(&"SC2120".to_string()), "{c:?}");
    // A named function still pairs its definition and callers.
    let c = codes("f() { echo $1; }\nf\n");
    assert!(c.contains(&"SC2119".to_string()) && c.contains(&"SC2120".to_string()), "{c:?}");
}

#[test]
fn read_prompt_form_defines_the_variable() {
    for s in [
        "read \"r?Proceed? \"\necho $r\n",
        "read -q \"k?Go? \"\necho $k\n",
        "read -r \"line?prompt: \" rest\necho $line $rest\n",
    ] {
        let c = codes(s);
        assert!(!c.contains(&"SC2154".to_string()), "{s:?}: {c:?}");
    }
    assert!(codes("read \"r?Proceed? \"\necho $nope\n").contains(&"SC2154".to_string()));
}

#[test]
fn read_prompt_variable_is_not_unused() {
    let c = codes("read -r \"q?go\"\necho $q\n");
    assert!(!c.contains(&"SC2034".to_string()), "{c:?}");
}
