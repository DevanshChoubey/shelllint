// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh pattern and subscript parser
// rules (lane P-B: paren pattern groups and glob flags, numeric ranges, glob
// qualifiers, case labels, subscripts). No ShellCheck counterpart.
//
// Every snippet marked "parses" was accepted by `/usr/bin/zsh -n`, and every
// "near miss" was rejected by it, when this file was written. Each construct
// has: positive snippets (parse under `--shell=zsh` with none of the 12 hard
// parse codes), a dialect guard per positive (`--shell=bash` gives exactly the
// codes the tree produced before the zsh rules; see
// `assert_same_as_bash_today`), and near misses that zsh itself rejects and
// that must still be reported as parse errors here.
//
// Construct ids are the research report's (Q3): C04 paren groups, C05 numeric
// ranges, C06 glob qualifiers, C07 case labels, C17 subscripts, C21 nested
// braces, C26 glob flags, C27 subscripts in `[[ ]]`, C29 `${(L)"${$(( ))}":1:5}`
// (the `[##36]` base prefix inside it belongs to lane P-C).

#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

use std::io::Write;
use std::process::{Command, Stdio};

/// Codes (with messages) for `script` under `--shell=<shell>`, in report order.
fn findings(shell: &str, script: &str) -> Vec<(u32, String)> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1"])
        .arg(format!("--shell={shell}"))
        .arg("-")
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    v["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["code"].as_u64().unwrap() as u32, c["message"].as_str().unwrap().to_string()))
        .collect()
}

fn assert_zsh_parses(script: &str) {
    assert_parses(script);
    // SC1089 ("parsing stopped") is not in the hard list but is as fatal.
    assert!(!zsh_codes(script).contains(&1089), "SC1089 for script:\n{script}");
}

fn assert_bash_today(script: &str, today: &[u32]) {
    assert_same_as_bash_today(script, today);
}

/// zsh rejects this (`zsh -n` fails); shelllint must report a parse error too.
fn assert_zsh_rejects(script: &str) {
    let got = zsh_codes(script);
    assert!(
        got.iter().any(|c| HARD_PARSE_CODES.contains(c) || *c == 1089),
        "expected a parse error under zsh, got {got:?} for script:\n{script}"
    );
}

// ---------------------------------------------------------------------------
// Per construct: positives, dialect guards, near misses (generated from the
// zsh -n verified snippet table).
// ---------------------------------------------------------------------------

#[test]
fn c04_parses_alt_group_then_star() {
    assert_zsh_parses("[[ $x == (a|b)* ]] && echo y\n");
}

#[test]
fn c04_guard_bash_unchanged_alt_group_then_star() {
    assert_bash_today("[[ $x == (a|b)* ]] && echo y\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c04_parses_two_groups_in_function() {
    assert_zsh_parses("f() { [[ $1 == (foo|bar)(baz|qux) ]] && return 0; return 1; }\n");
}

#[test]
fn c04_guard_bash_unchanged_two_groups_in_function() {
    assert_bash_today(
        "f() { [[ $1 == (foo|bar)(baz|qux) ]] && return 0; return 1; }\n",
        &[1009, 1073, 1027, 1036, 1072],
    );
}

#[test]
fn c04_parses_hash_and_double_hash_repeat() {
    assert_zsh_parses("[[ $x == (ab)##c || $x != (a|b)#d ]]\n");
}

#[test]
fn c04_guard_bash_unchanged_hash_and_double_hash_repeat() {
    assert_bash_today("[[ $x == (ab)##c || $x != (a|b)#d ]]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c04_parses_glob_subst_before_group() {
    assert_zsh_parses("[[ $x == ${~v}(a|b) ]]\n");
}

#[test]
fn c04_guard_bash_unchanged_glob_subst_before_group() {
    assert_bash_today("[[ $x == ${~v}(a|b) ]]\n", &[1073, 1036, 1072]);
}

#[test]
fn c04_parses_in_pipeline_block() {
    assert_zsh_parses("echo hi | { read -r l; [[ $l == (x|y)z ]] && echo m; }\n");
}

#[test]
fn c04_guard_bash_unchanged_in_pipeline_block() {
    assert_bash_today("echo hi | { read -r l; [[ $l == (x|y)z ]] && echo m; }\n", &[1009, 1073, 1027, 1036, 1072]);
}

#[test]
fn c04_parses_nested_group() {
    assert_zsh_parses("[[ $x == a(b|(c|d))e ]]\n");
}

#[test]
fn c04_guard_bash_unchanged_nested_group() {
    assert_bash_today("[[ $x == a(b|(c|d))e ]]\n", &[1073, 1036, 1072]);
}

#[test]
fn c04_near_miss_is_rejected_unclosed_group() {
    assert_zsh_rejects("[[ $x == (a|b ]]\n");
}

#[test]
fn c04_near_miss_is_rejected_bare_bar_without_group() {
    assert_zsh_rejects("[[ $x == a|b ]]\n");
}

#[test]
fn c05_parses_any_number() {
    assert_zsh_parses("[[ $x == <-> ]]\n");
}

#[test]
fn c05_guard_bash_unchanged_any_number() {
    assert_bash_today("[[ $x == <-> ]]\n", &[1073, 1027, 1072]);
}

#[test]
fn c05_parses_closed_range() {
    assert_zsh_parses("[[ $x == <0-65535> ]]\n");
}

#[test]
fn c05_guard_bash_unchanged_closed_range() {
    assert_bash_today("[[ $x == <0-65535> ]]\n", &[1073, 1027, 1072]);
}

#[test]
fn c05_parses_open_upper_in_if() {
    assert_zsh_parses("if [[ $x == <1-> ]]; then echo n; fi\n");
}

#[test]
fn c05_guard_bash_unchanged_open_upper_in_if() {
    assert_bash_today("if [[ $x == <1-> ]]; then echo n; fi\n", &[1009, 1073, 1027, 1072]);
}

#[test]
fn c05_parses_range_mid_word() {
    assert_zsh_parses("[[ $x == v<->.<->* ]]\n");
}

#[test]
fn c05_guard_bash_unchanged_range_mid_word() {
    assert_bash_today("[[ $x == v<->.<->* ]]\n", &[1073, 1072]);
}

#[test]
fn c05_parses_open_lower_and_upper() {
    assert_zsh_parses("[[ $x == <-9> ]] || [[ $x == <5-> ]]\n");
}

#[test]
fn c05_guard_bash_unchanged_open_lower_and_upper() {
    assert_bash_today("[[ $x == <-9> ]] || [[ $x == <5-> ]]\n", &[1073, 1027, 1072]);
}

#[test]
fn c05_parses_in_function_with_arith() {
    assert_zsh_parses("f() { [[ $1 == <-> ]] && echo $(( $1 + 1 )); }\n");
}

#[test]
fn c05_guard_bash_unchanged_in_function_with_arith() {
    assert_bash_today("f() { [[ $1 == <-> ]] && echo $(( $1 + 1 )); }\n", &[1009, 1073, 1027, 1072]);
}

#[test]
fn c05_near_miss_is_rejected_lone_less_than() {
    assert_zsh_rejects("[[ $x == < ]]\n");
}

#[test]
fn c06_parses_nullglob_qualifier() {
    assert_zsh_parses("print x(N)\n");
}

#[test]
fn c06_guard_bash_unchanged_nullglob_qualifier() {
    assert_bash_today("print x(N)\n", &[1036, 1088]);
}

#[test]
fn c06_parses_recursive_qualifier_with_modifier() {
    assert_zsh_parses("print **/*(.N:t)\n");
}

#[test]
fn c06_guard_bash_unchanged_recursive_qualifier_with_modifier() {
    assert_bash_today("print **/*(.N:t)\n", &[2035]);
}

#[test]
fn c06_parses_qualifier_in_for_list() {
    assert_zsh_parses("for f in **/x(-.N:h); do echo $f; done\n");
}

#[test]
fn c06_guard_bash_unchanged_qualifier_in_for_list() {
    assert_bash_today("for f in **/x(-.N:h); do echo $f; done\n", &[1073, 1036, 1058, 1072]);
}

#[test]
fn c06_parses_star_ext_qualifier() {
    assert_zsh_parses("print -- *.txt(N.)\n");
}

#[test]
fn c06_guard_bash_unchanged_star_ext_qualifier() {
    assert_bash_today("print -- *.txt(N.)\n", &[1036, 1088]);
}

#[test]
fn c06_parses_dir_qualifier() {
    assert_zsh_parses("ls ~/x/*(/N)\n");
}

#[test]
fn c06_guard_bash_unchanged_dir_qualifier() {
    assert_bash_today("ls ~/x/*(/N)\n", &[]);
}

#[test]
fn c06_parses_order_and_subrange() {
    assert_zsh_parses("print x(om[1,3])\n");
}

#[test]
fn c06_guard_bash_unchanged_order_and_subrange() {
    assert_bash_today("print x(om[1,3])\n", &[1036, 1088]);
}

#[test]
fn c06_parses_after_glob_subst_in_array() {
    assert_zsh_parses("f() { local -a l; l=( ${~1}(N) ); print -l $l; }\n");
}

#[test]
fn c06_guard_bash_unchanged_after_glob_subst_in_array() {
    assert_bash_today("f() { local -a l; l=( ${~1}(N) ); print -l $l; }\n", &[2206, 2296, 1036, 2128, 2086]);
}

#[test]
fn c06_parses_star_qualifier_mtime() {
    assert_zsh_parses("print *(Nm-1)\n");
}

#[test]
fn c06_guard_bash_unchanged_star_qualifier_mtime() {
    assert_bash_today("print *(Nm-1)\n", &[]);
}

#[test]
fn c06_near_miss_is_rejected_extra_close_paren() {
    assert_zsh_rejects("print x(N)))\n");
}

#[test]
fn c07_parses_group_then_star_label() {
    assert_zsh_parses("case $x in (a|b)*) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_group_then_star_label() {
    assert_bash_today("case $x in (a|b)*) :;; esac\n", &[1009, 1073, 1072, 1074]);
}

#[test]
fn c07_parses_number_label() {
    assert_zsh_parses("case $x in <->) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_number_label() {
    assert_bash_today("case $x in <->) :;; esac\n", &[1073, 1072]);
}

#[test]
fn c07_parses_two_group_labels() {
    assert_zsh_parses("case $x in (a|b)) :;; (c|d)) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_two_group_labels() {
    assert_bash_today("case $x in (a|b)) :;; (c|d)) :;; esac\n", &[1009, 1073, 1072]);
}

#[test]
fn c07_parses_range_star_label() {
    assert_zsh_parses("case $x in <1-9>*) :;; *) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_range_star_label() {
    assert_bash_today("case $x in <1-9>*) :;; *) :;; esac\n", &[1073, 1072]);
}

#[test]
fn c07_parses_optional_paren_label() {
    assert_zsh_parses("case $x in (a|b) echo y;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_optional_paren_label() {
    assert_bash_today("case $x in (a|b) echo y;; esac\n", &[2154]);
}

#[test]
fn c07_parses_groups_joined_by_bar() {
    assert_zsh_parses("case $x in (foo)|(bar)) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_groups_joined_by_bar() {
    assert_bash_today("case $x in (foo)|(bar)) :;; esac\n", &[1009, 1073, 1072]);
}

#[test]
fn c07_parses_multiline_in_function() {
    assert_zsh_parses(
        "f() {\n  case $1 in\n    (a|b)*) echo ab ;;\n    <->) echo num ;;\n    *) echo other ;;\n  esac\n}\n",
    );
}

#[test]
fn c07_guard_bash_unchanged_multiline_in_function() {
    assert_bash_today(
        "f() {\n  case $1 in\n    (a|b)*) echo ab ;;\n    <->) echo num ;;\n    *) echo other ;;\n  esac\n}\n",
        &[1009, 1073, 1072, 1074],
    );
}

#[test]
fn c07_parses_nested_group_label() {
    assert_zsh_parses("case $x in ((a|b)c)) :;; esac\n");
}

#[test]
fn c07_guard_bash_unchanged_nested_group_label() {
    assert_bash_today("case $x in ((a|b)c)) :;; esac\n", &[1009, 1073, 1036, 1072]);
}

#[test]
fn c07_near_miss_is_rejected_missing_esac() {
    assert_zsh_rejects("case $x in (a|b)*) :;;\n");
}

#[test]
fn c07_near_miss_is_rejected_label_without_close_paren() {
    assert_zsh_rejects("case $x in (a|b)* :;; esac\n");
}

#[test]
fn c17_parses_flag_i() {
    assert_zsh_parses("echo $a[(I)x]\n");
}

#[test]
fn c17_guard_bash_unchanged_flag_i() {
    assert_bash_today("echo $a[(I)x]\n", &[1087, 2154, 2086, 1036, 1088]);
}

#[test]
fn c17_parses_flag_ie_with_expansion() {
    assert_zsh_parses("echo $a[(Ie)$w]\n");
}

#[test]
fn c17_guard_bash_unchanged_flag_ie_with_expansion() {
    assert_bash_today("echo $a[(Ie)$w]\n", &[1087, 2154, 2086, 1036, 1088]);
}

#[test]
fn c17_parses_flag_s_separator() {
    assert_zsh_parses("echo $a[(s: :w)2]\n");
}

#[test]
fn c17_guard_bash_unchanged_flag_s_separator() {
    assert_bash_today("echo $a[(s: :w)2]\n", &[1087, 2154, 2086, 1036, 1088]);
}

#[test]
fn c17_parses_plain_index() {
    assert_zsh_parses("echo $a[i]\n");
}

#[test]
fn c17_guard_bash_unchanged_plain_index() {
    assert_bash_today("echo $a[i]\n", &[1087, 2154, 2086]);
}

#[test]
fn c17_parses_length_negative_and_range() {
    assert_zsh_parses("echo $a[$#a] $a[-1] $a[1,2]\n");
}

#[test]
fn c17_guard_bash_unchanged_length_negative_and_range() {
    assert_bash_today("echo $a[$#a] $a[-1] $a[1,2]\n", &[1087, 2154, 2086, 1087, 2086, 1087, 2086]);
}

#[test]
fn c17_parses_inside_double_quotes() {
    assert_zsh_parses("echo \"$a[(r)foo]\"\n");
}

#[test]
fn c17_guard_bash_unchanged_inside_double_quotes() {
    assert_bash_today("echo \"$a[(r)foo]\"\n", &[1087, 2154]);
}

#[test]
fn c17_parses_assignment_value() {
    assert_zsh_parses("x=$a[(r)foo]\n");
}

#[test]
fn c17_guard_bash_unchanged_assignment_value() {
    assert_bash_today("x=$a[(r)foo]\n", &[2034, 1087, 2154, 1036, 1088]);
}

#[test]
fn c17_parses_special_param_in_function() {
    assert_zsh_parses("f() { local i=1; print -r -- $path[$i] $path[(i)/usr/bin]; }\n");
}

#[test]
fn c17_guard_bash_unchanged_special_param_in_function() {
    assert_bash_today(
        "f() { local i=1; print -r -- $path[$i] $path[(i)/usr/bin]; }\n",
        &[1009, 1073, 1036, 1056, 1072],
    );
}

#[test]
fn c17_parses_braced_and_unbraced_together() {
    assert_zsh_parses("echo ${a[(r)x]} $a[(r)x]\n");
}

#[test]
fn c17_guard_bash_unchanged_braced_and_unbraced_together() {
    assert_bash_today("echo ${a[(r)x]} $a[(r)x]\n", &[2154, 2086, 1087, 2086, 1036, 1088]);
}

#[test]
fn c17_near_miss_is_rejected_unclosed_flag_subscript() {
    assert_zsh_rejects("echo $a[(I)x\n");
}

#[test]
fn c17_near_miss_is_rejected_unclosed_plain_subscript() {
    assert_zsh_rejects("echo $a[1\n");
}

#[test]
fn c21_parses_reverse_range_nested() {
    assert_zsh_parses("echo ${(@)${:-{$#p..1}}/(#m)*/${p[$MATCH]}}\n");
}

#[test]
fn c21_guard_bash_unchanged_reverse_range_nested() {
    assert_bash_today("echo ${(@)${:-{$#p..1}}/(#m)*/${p[$MATCH]}}\n", &[2086, 2296, 1036, 1088]);
}

#[test]
fn c21_parses_nested_match_replace() {
    assert_zsh_parses("echo ${(@)${:-a b}/(#m)*/<$MATCH>}\n");
}

#[test]
fn c21_guard_bash_unchanged_nested_match_replace() {
    assert_bash_today("echo ${(@)${:-a b}/(#m)*/<$MATCH>}\n", &[2086, 2296]);
}

#[test]
fn c21_parses_nested_braces_default() {
    assert_zsh_parses("echo ${:-{a,b}}\n");
}

#[test]
fn c21_guard_bash_unchanged_nested_braces_default() {
    assert_bash_today("echo ${:-{a,b}}\n", &[2086, 1083]);
}

#[test]
fn c21_parses_double_nested_in_function() {
    assert_zsh_parses("f() { print -l ${(@)${(@)a}/(#m)*/x$MATCH}; }\n");
}

#[test]
fn c21_guard_bash_unchanged_double_nested_in_function() {
    assert_bash_today("f() { print -l ${(@)${(@)a}/(#m)*/x$MATCH}; }\n", &[2086, 2296, 2296]);
}

#[test]
fn c21_near_miss_is_rejected_unbalanced_brace_default() {
    assert_zsh_rejects("echo ${a:-{x}\n");
}

#[test]
fn c26_parses_flag_i() {
    assert_zsh_parses("[[ a == (#i)A ]]\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_i() {
    assert_bash_today("[[ a == (#i)A ]]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c26_parses_flag_b_with_match() {
    assert_zsh_parses("[[ abc == (#b)(a)(b)c ]] && echo $match[1]\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_b_with_match() {
    assert_bash_today("[[ abc == (#b)(a)(b)c ]] && echo $match[1]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c26_parses_flag_m() {
    assert_zsh_parses("[[ a == (#m)a ]]\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_m() {
    assert_bash_today("[[ a == (#m)a ]]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c26_parses_flag_q_in_arg() {
    assert_zsh_parses("print -l (#q)*\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_q_in_arg() {
    assert_bash_today("print -l (#q)*\n", &[1036, 1088]);
}

#[test]
fn c26_parses_flag_i_in_arg() {
    assert_zsh_parses("setopt extendedglob; print -- (#i)foo*\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_i_in_arg() {
    assert_bash_today("setopt extendedglob; print -- (#i)foo*\n", &[1036, 1088]);
}

#[test]
fn c26_parses_flag_i_then_group() {
    assert_zsh_parses("[[ $x == (#i)(a|b) ]]\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_i_then_group() {
    assert_bash_today("[[ $x == (#i)(a|b) ]]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c26_parses_flag_b_two_captures() {
    assert_zsh_parses("[[ ab-cd == (#b)(*)-(*) ]] && print $match[2]\n");
}

#[test]
fn c26_guard_bash_unchanged_flag_b_two_captures() {
    assert_bash_today("[[ ab-cd == (#b)(*)-(*) ]] && print $match[2]\n", &[1073, 1027, 1036, 1072]);
}

#[test]
fn c26_near_miss_is_rejected_unclosed_flag() {
    assert_zsh_rejects("[[ a == (#i A ]]\n");
}

#[test]
fn c27_parses_flag_r_lhs() {
    assert_zsh_parses("[[ $a[(r)x] == x ]]\n");
}

#[test]
fn c27_guard_bash_unchanged_flag_r_lhs() {
    assert_bash_today("[[ $a[(r)x] == x ]]\n", &[1073, 1036, 1072]);
}

#[test]
fn c27_parses_flag_i_unary() {
    assert_zsh_parses("[[ -n $a[(I)x] ]]\n");
}

#[test]
fn c27_guard_bash_unchanged_flag_i_unary() {
    assert_bash_today("[[ -n $a[(I)x] ]]\n", &[1073, 1036, 1072]);
}

#[test]
fn c27_parses_flag_ie_numeric_compare() {
    assert_zsh_parses("if [[ $a[(Ie)$w] -gt 0 ]]; then :; fi\n");
}

#[test]
fn c27_guard_bash_unchanged_flag_ie_numeric_compare() {
    assert_bash_today("if [[ $a[(Ie)$w] -gt 0 ]]; then :; fi\n", &[1009, 1073, 1036, 1072]);
}

#[test]
fn c27_parses_two_index_terms() {
    assert_zsh_parses("[[ $a[1] == b || $a[-1] == c ]]\n");
}

#[test]
fn c27_guard_bash_unchanged_two_index_terms() {
    assert_bash_today("[[ $a[1] == b || $a[-1] == c ]]\n", &[1087, 2203, 2154, 1087, 2203]);
}

#[test]
fn c27_parses_flag_i_with_param_in_function() {
    assert_zsh_parses("f() { [[ $a[(i)$1] -le $#a ]] && echo found; }\n");
}

#[test]
fn c27_guard_bash_unchanged_flag_i_with_param_in_function() {
    assert_bash_today("f() { [[ $a[(i)$1] -le $#a ]] && echo found; }\n", &[1009, 1073, 1036, 1072]);
}

#[test]
fn c27_near_miss_is_rejected_missing_rhs() {
    assert_zsh_rejects("[[ $a[1] == ]]\n");
}

#[test]
fn c29_parses_lower_flag_quoted_inner_c29() {
    assert_zsh_parses("echo ${(L)\"${x}\":1:5}\n");
}

#[test]
fn c29_guard_bash_unchanged_lower_flag_quoted_inner_c29() {
    assert_bash_today("echo ${(L)\"${x}\":1:5}\n", &[2086, 2296]);
}

#[test]
fn c29_parses_arith_base_inside() {
    assert_zsh_parses("echo ${(L)\"${$(( 36#123 ))}\":1:5}\n");
}

#[test]
fn c29_guard_bash_unchanged_arith_base_inside() {
    assert_bash_today("echo ${(L)\"${$(( 36#123 ))}\":1:5}\n", &[2086, 2296, 2082]);
}

#[test]
fn c29_parses_in_local_assignment() {
    assert_zsh_parses("f() { local s=${(L)\"${$(( 16#ff ))}\":0:2}; echo $s; }\n");
}

#[test]
fn c29_guard_bash_unchanged_in_local_assignment() {
    assert_bash_today("f() { local s=${(L)\"${$(( 16#ff ))}\":0:2}; echo $s; }\n", &[2296, 2082, 2086]);
}

// ---------------------------------------------------------------------------
// Structure: what the parser builds for each construct (`--dump-ast`, zsh).
// ---------------------------------------------------------------------------

fn dump_ast(shell: &str, script: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "--dump-ast"])
        .arg(format!("--shell={shell}"))
        .arg("-")
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(script.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    String::from_utf8(o.stdout).unwrap()
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

#[test]
fn structure_c17_subscript_is_one_braced_expansion_not_a_glob() {
    let a = dump_ast("zsh", "echo $a[(I)x]\n");
    assert_eq!(count(&a, "\"type\":\"T_DollarBraced\""), 1, "{a}");
    assert_eq!(count(&a, "\"type\":\"T_Glob\""), 0, "{a}");
    assert!(a.contains("\"value\":\"a[(I)x]\""), "the name and subscript stay in one literal: {a}");
}

#[test]
fn structure_c17_expansion_inside_subscript_is_its_own_expansion() {
    let a = dump_ast("zsh", "echo $a[(Ie)$w]x\n");
    assert_eq!(count(&a, "\"type\":\"T_DollarBraced\""), 2, "outer and $w: {a}");
    assert!(a.contains("\"value\":\"a[(Ie)\""), "{a}");
    assert!(a.contains("\"value\":\"]\""), "{a}");
    assert!(a.contains("\"value\":\"x\""), "text after the subscript is a separate part: {a}");
}

#[test]
fn structure_c17_nested_brackets_stay_in_the_subscript() {
    let a = dump_ast("zsh", "echo $a[$b[1]] c\n");
    assert_eq!(count(&a, "\"type\":\"T_DollarBraced\""), 2, "{a}");
}

#[test]
fn structure_c17_unsubscripted_variable_is_unchanged() {
    let z = dump_ast("zsh", "echo $a ${b} \"$c\"\n");
    let b = dump_ast("bash", "echo $a ${b} \"$c\"\n");
    assert_eq!(z, b);
}

#[test]
fn structure_c04_group_is_an_extglob_with_empty_operator() {
    let a = dump_ast("zsh", "[[ $x == (a|b)* ]]\n");
    assert_eq!(count(&a, "\"type\":\"T_Extglob\""), 1, "{a}");
    assert_eq!(count(&a, "\"type\":\"T_Glob\""), 1, "the star: {a}");
}

#[test]
fn structure_c06_qualifier_after_a_word_is_a_group() {
    let a = dump_ast("zsh", "print x(N)\n");
    assert_eq!(count(&a, "\"type\":\"T_Extglob\""), 1, "{a}");
}

#[test]
fn structure_c05_numeric_range_is_one_glob() {
    let a = dump_ast("zsh", "[[ $x == <1-5> ]]\n");
    assert_eq!(count(&a, "\"type\":\"T_Glob\""), 1, "{a}");
    assert!(a.contains("\"value\":\"<1-5>\""), "{a}");
}

#[test]
fn structure_c07_optional_paren_is_not_part_of_the_label() {
    // `(a|b) cmd` : the paren is case syntax, so the label is `a|b` (two
    // patterns, no group node).
    let a = dump_ast("zsh", "case $x in (a|b) echo y;; esac\n");
    assert_eq!(count(&a, "\"type\":\"T_Extglob\""), 0, "{a}");
    assert_eq!(count(&a, "\"type\":\"T_CaseExpression\""), 1, "{a}");
}

#[test]
fn structure_c07_paren_followed_by_pattern_text_is_a_group() {
    let a = dump_ast("zsh", "case $x in (a|b)*) echo y;; esac\n");
    assert_eq!(count(&a, "\"type\":\"T_Extglob\""), 1, "{a}");
}

#[test]
fn structure_c07_double_paren_label_has_one_group() {
    // `((a|b)c)` : optional paren, then the group-less pattern `(a|b)c`.
    let a = dump_ast("zsh", "case $x in ((a|b)c) echo y;; esac\n");
    assert_eq!(count(&a, "\"type\":\"T_Extglob\""), 1, "{a}");
}

#[test]
fn structure_c21_balanced_braces_stay_inside_the_expansion() {
    let a = dump_ast("zsh", "echo ${:-{a,b}}\n");
    assert_eq!(count(&a, "\"type\":\"T_DollarBraced\""), 1, "{a}");
    assert_eq!(count(&a, "\"type\":\"T_BraceExpansion\""), 0, "{a}");
}

// ---------------------------------------------------------------------------
// Things the new rules must leave alone under zsh.
// ---------------------------------------------------------------------------

#[test]
fn regress_functions_and_subshells_still_parse_under_zsh() {
    for s in [
        "f() { echo hi; }\n",
        "f () { :; }\n",
        "function g() { :; }\n",
        "function h { :; }\n",
        "(cd /tmp; ls)\n",
        "f() ( echo sub )\n",
        "echo hi | (cat)\n",
        "foo & (bar)\n",
        "if (( x )); then :; fi\n",
        "echo a$((1+2)) $(( (1+2)*3 ))\n",
        "echo $(date) a$(date)b\n",
        "[[ ( $a == b ) && ( $c == d ) ]]\n",
        "[[ ( -n $x ) ]]\n",
        "[[ ( $a == (x|y) ) ]]\n",
    ] {
        assert_zsh_parses(s);
    }
}

#[test]
fn regress_array_assignments_are_not_groups() {
    for s in [
        "foo=(1 2); echo ${foo[1]}\n",
        "typeset -a y=(c d); y+=(e); echo $y\n",
        "local -A m=(k v); echo $m[k]\n",
        "x=(a b); x+=(c)\n",
    ] {
        assert_zsh_parses(s);
    }
}

#[test]
fn regress_process_substitution_is_not_a_numeric_range() {
    assert_zsh_parses("diff <(echo a) <(echo b)\n");
    assert_zsh_parses("cat <(echo a) >(cat)\n");
}

#[test]
fn regress_case_labels_without_groups_still_parse() {
    assert_zsh_parses("case $x in a|b) echo ok;; esac\n");
    assert_zsh_parses("case x in a) (cd /; ls);; esac\n");
    assert_zsh_parses("case $x in\n  a) echo 1 ;;\n  *) echo 2 ;;\nesac\n");
}

#[test]
fn regress_echo_double_bracket_is_a_plain_word() {
    assert_zsh_parses("echo [[ a ]]\n");
}

#[test]
fn guard_non_zsh_shells_all_still_reject_the_zsh_forms() {
    // The new rules are gated on zsh: sh, dash, ksh and bash keep failing to
    // parse `[[ $x == (a|b)* ]]`-style words exactly as before.
    for sh in ["sh", "dash", "bash", "ksh"] {
        let hard = findings(sh, "print x(N)\n");
        let hard_bash = findings("bash", "print x(N)\n");
        assert_eq!(hard, hard_bash, "{sh} differs from bash");
    }
}

#[test]
fn guard_bash_keeps_sc1087_for_unbraced_subscript_zsh_drops_it() {
    // bash: `$a[1]` is `$a` followed by a glob, and SC1087 says so.
    assert!(bash_codes("echo $a[1]\n").contains(&1087));
    // zsh: a subscript, nothing to warn about.
    assert!(!zsh_codes("echo $a[1]\n").contains(&1087));
}

#[test]
fn escaped_close_bracket_operand_is_not_a_missing_space() {
    // zsh-syntax-highlighting shape: `[[ $BUFFER[$2] == \] ]]`.
    assert_zsh_parses("[[ $BUFFER[$2] == \\] ]]\n");
    // bash still reports SC1020 for the same text.
    assert!(bash_codes("[[ $x == \\] ]]\n").contains(&1020));
}

#[test]
fn glob_flag_word_after_a_command_name_is_not_a_function_header() {
    // `print (#q)*` once looked like `name (params)`; in zsh it is a command
    // with a glob-flag word. Bash keeps the SC1065 reading.
    assert_zsh_parses("print -l (#q)*\n");
    assert!(!zsh_codes("print -l (#q)*\n").contains(&1065));
    assert!(bash_codes("print (#q)*\n").contains(&1065));
}

#[test]
fn unclosed_subscript_reports_sc1073_in_zsh() {
    assert!(zsh_codes("echo $a[(I)x\n").contains(&1073));
    assert!(zsh_codes("echo $a[1\n").contains(&1073));
    assert!(!bash_codes("echo $a[1\n").contains(&1073));
}
