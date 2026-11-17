// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// The four classes of design section 4, each as a behaviour:
//   (a) unchanged: one zsh repro proving the code still fires,
//   (b) fires only when the option makes it true: on, off, explicit `${=x}` / `${~x}`,
//       and the check's own non-splitting conditions,
//   (c) trigger changed for zsh: special parameters, hook registrations, patterns,
//   (d) off, because no zsh-true form exists: silent under zsh, unchanged under bash.
// The option spellings and scopes themselves are in `options.rs`.
//
// Facts used here were checked with zsh 5.9 (`zsh -fc`):
//   `x="a b"; n(){print $#}; n $x` prints 1, with `setopt shwordsplit` 2, `${=x}` 2.
//   `x='*'; [[ usr == $x ]]` does not match, with `setopt globsubst` it matches; `${~x}` globs.
//   `x=; [ -n $x ]` is true (empty unquoted expansion removed); `[ $x = y ]` is a parse error.
//   `typeset -A m; m=(a 1 b 2)` is native; `n=3; echo {1..$n}` prints 1 2 3;
//   `0=foo; echo $0` prints foo; `1=bar; echo $1` prints bar; `local x=1` works at top level.
//   `m[1][2]=3` parses (`zsh -n`) but at run time is "no matches found": zsh has no multi-
//   dimensional arrays; SC2180 is silent under zsh because the design lists it in class (d).
//   `echo hi | read v; echo $v` prints hi (last pipeline element runs in the current shell);
//   `read v <<<hi | cat; echo $v` prints nothing.
use crate::support::*;

// ---- (a) unchanged ----------------------------------------------------------------------

code_case!(now, a_sc2139_alias_with_expansion, 2139, "alias a=\"echo $1\"\n", [1]);
code_case!(now, a_sc2155_local_with_command_substitution, 2155, "f() { local x=$(false); echo \"$x\"; }\nf\n", [1]);
code_case!(now, a_sc2046_unquoted_command_substitution_still_splits, 2046, "echo $(date)\n", [1]);
code_case!(
    now,
    a_sc2207_array_from_command_substitution_splits_by_default,
    2207,
    "a=($(date))\necho \"${a[@]}\"\n",
    [1]
);
code_case!(now, a_sc2006_backticks, 2006, "x=`date`\necho \"$x\"\n", [1]);
code_case!(now, a_sc2091_executing_command_output, 2091, "$(echo ls)\n", [1]);
code_case!(now, a_sc2181_checking_dollar_question, 2181, "foo\nif [ $? -eq 0 ]; then :; fi\n", [2]);
code_case!(now, a_sc2125_brace_in_assignment, 2125, "x={1..3}\necho \"$x\"\n", [1]);
code_case!(now, a_sc2153_possible_misspelling, 2153, "MY_VAR=1\necho \"$MY_VAR\"\necho \"$MYVAR\"\n", [3]);
code_case!(now, a_sc2164_cd_without_guard, 2164, "cd /x\nls\n", [1]);
code_case!(now, a_sc2202_glob_as_test_operand, 2202, "[ ./*.txt -nt x ]\n", [1]);
code_case!(now, a_sc2215_flag_as_command_name, 2215, "-f foo\n", [1]);
code_case!(now, a_sc2157_constant_string_in_test, 2157, "[ -z \"foo\" ]\n", [1]);
code_case!(now, a_sc2179_appending_string_to_array, 2179, "a=(x)\na+=y\necho \"${a[@]}\"\n", [2]);
code_case!(now, a_sc2145_array_mixed_with_string, 2145, "echo \"foo$@\"\n", [1]);

// ---- (b) shwordsplit ---------------------------------------------------------------------

code_case!(now, b_sc2086_silent_by_default, 2086, "echo $1\n", []);
code_case!(now, b_sc2086_fires_with_shwordsplit, 2086, "setopt shwordsplit\necho $1\n", [2]);
code_case!(now, b_sc2086_fires_on_explicit_equals_flag_by_default, 2086, "echo ${=1}\n", [1]);
// A `for` list is where splitting is wanted, so (as for `$x` in bash) it stays silent even with the
// explicit flag; the same word as a command argument fires (previous test).
code_case!(now, b_sc2086_explicit_equals_flag_in_a_for_list_stays_silent_as_in_bash, 2086, "for w in ${=1}; do :; done\n", []);
code_case!(now, b_sc2086_fires_on_explicit_tilde_flag_by_default, 2086, "echo ${~1}\n", [1]);
code_case!(now, b_sc2086_quoted_stays_silent_with_shwordsplit, 2086, "setopt shwordsplit\necho \"$1\"\n", []);
code_case!(now, b_sc2086_assignment_stays_silent_with_shwordsplit, 2086, "setopt shwordsplit\nx=$1\n", []);
code_case!(now, b_sc2086_double_bracket_stays_silent_with_shwordsplit, 2086, "setopt shwordsplit\n[[ $1 = a ]]\n", []);
code_case!(
    now,
    b_sc2086_case_word_stays_silent_with_shwordsplit,
    2086,
    "setopt shwordsplit\ncase $1 in a) :;; esac\n",
    []
);
code_case!(now, b_sc2070_empty_removal_in_single_bracket_remains, 2070, "[ -n $1 ]\n", [1]);

code_case!(now, b_sc2206_silent_by_default, 2206, "a=($1)\necho \"${a[@]}\"\n", []);
code_case!(now, b_sc2206_fires_with_shwordsplit, 2206, "setopt shwordsplit\na=($1)\necho \"${a[@]}\"\n", [2]);
code_case!(now, b_sc2206_fires_on_explicit_equals_flag, 2206, "a=(${=1})\necho \"${a[@]}\"\n", [1]);
code_case!(
    now,
    b_sc2206_quoted_stays_silent_with_shwordsplit,
    2206,
    "setopt shwordsplit\na=(\"$1\")\necho \"${a[@]}\"\n",
    []
);
code_case!(
    now,
    b_sc2207_fires_by_default_and_with_shwordsplit,
    2207,
    "setopt shwordsplit\na=($(date))\necho \"${a[@]}\"\n",
    [2]
);

code_case!(now, b_sc2048_silent_by_default, 2048, "for i in $*; do :; done\n", []);
code_case!(now, b_sc2048_fires_with_shwordsplit, 2048, "setopt shwordsplit\nfor i in $*; do :; done\n", [2]);
code_case!(
    now,
    b_sc2048_quoted_at_stays_silent_with_shwordsplit,
    2048,
    "setopt shwordsplit\nfor i in \"$@\"; do :; done\n",
    []
);
code_case!(now, b_sc2068_silent_by_default, 2068, "foo $@\n", []);
code_case!(now, b_sc2068_fires_with_shwordsplit, 2068, "setopt shwordsplit\nfoo $@\n", [2]);
code_case!(now, b_sc2068_quoted_stays_silent_with_shwordsplit, 2068, "setopt shwordsplit\nfoo \"$@\"\n", []);
code_case!(now, b_sc2223_silent_by_default, 2223, ": ${x:=$1}\n", []);
code_case!(now, b_sc2223_fires_with_shwordsplit, 2223, "setopt shwordsplit\n: ${x:=$1}\n", [2]);
code_case!(now, b_sc2223_quoted_stays_silent_with_shwordsplit, 2223, "setopt shwordsplit\n: \"${x:=$1}\"\n", []);

// ---- (b) globsubst (on/off spellings are in options.rs) ----------------------------------

code_case!(now, b_sc2053_fires_on_explicit_tilde_flag_by_default, 2053, "[[ $1 == ${~2} ]]\n", [1]);
code_case!(now, b_sc2254_fires_on_explicit_tilde_flag_by_default, 2254, "case $1 in ${~2}) :;; esac\n", [1]);
code_case!(now, b_sc2295_fires_on_explicit_tilde_flag_by_default, 2295, "x=\"${1#${~2}}\"\n", [1]);
code_case!(
    now,
    b_sc2254_literal_pattern_stays_silent_with_globsubst,
    2254,
    "setopt globsubst\ncase $1 in a*) :;; esac\n",
    []
);

// ---- (c) special parameters, hooks, registrations, patterns ------------------------------

/// Lower-case parameters zsh defines itself (spec section 4 (c) and research Q5).
const SPECIAL_PARAMS: &[&str] = &[
    "commands",
    "functions",
    "history",
    "words",
    "widgets",
    "terminfo",
    "langinfo",
    "pipestatus",
    "fpath",
    "path",
    "match",
    "mbegin",
    "mend",
    "reply",
    "precmd_functions",
    "preexec_functions",
    "chpwd_functions",
    "zshexit_functions",
    "reset_color",
    "fg",
    "bg",
    "aliases",
    "parameters",
];

case!(now, c_sc2154_zsh_special_parameters_are_not_unassigned, {
    let mut offenders = vec![];
    for n in SPECIAL_PARAMS {
        let got = zsh(&format!("echo \"${{{n}}}\"\n"));
        if has(&got, 2154) {
            offenders.push(*n);
        }
    }
    assert!(offenders.is_empty(), "SC2154 on zsh special parameters: {offenders:?}");
});

case!(now, c_sc2154_zsh_special_parameters_with_subscripts, {
    for s in [
        "echo \"${commands[ls]}\"\n",
        "echo \"${functions[f]}\"\n",
        "echo \"${(k)widgets}\"\n",
        "echo \"${match[1]}\" \"${mbegin[1]}\" \"${mend[1]}\"\n",
        "echo \"${reply[1]}\"\n",
        "echo \"${fg[red]}${reset_color}\"\n",
        "echo \"${pipestatus[1]}\"\n",
        "echo \"${terminfo[colors]}\"\n",
    ] {
        let got = zsh(s);
        assert!(!has(&got, 2154), "SC2154 under zsh for {s:?}: {:?}", pairs(&got));
    }
});

case!(now, c_sc2034_assigning_zsh_special_parameters_is_not_unused, {
    let mut offenders = vec![];
    for n in [
        "reply",
        "REPLY",
        "precmd_functions",
        "preexec_functions",
        "chpwd_functions",
        "zshexit_functions",
        "fpath",
        "path",
        "PROMPT",
        "RPROMPT",
        "ZSH_THEME_GIT_PROMPT_PREFIX",
        "TMPPREFIX",
        "match",
        "MATCH",
    ] {
        let got = zsh(&format!("{n}=x\n"));
        if has(&got, 2034) {
            offenders.push(n);
        }
    }
    assert!(offenders.is_empty(), "SC2034 on assignments to zsh special parameters: {offenders:?}");
});

case!(now, c_sc2034_hook_array_append_is_not_unused, {
    let got = zsh("precmd_functions+=(my_hook)\nmy_hook() { :; }\n");
    assert!(!has(&got, 2034), "{:?}", pairs(&got));
});

case!(now, c_sc2154_ordinary_unassigned_variable_still_fires_under_zsh, {
    assert_eq!(lines(&zsh("echo \"$undefined_thing\"\n"), 2154), [1]);
});
case!(now, c_sc2154_special_parameter_names_still_fire_under_bash, {
    assert_eq!(lines(&shell("bash", "echo \"$commands\"\n"), 2154), [1]);
});
case!(now, c_sc2034_unused_assignment_still_fires_under_zsh_for_ordinary_names, {
    assert_eq!(lines(&zsh("unused_thing=x\n"), 2034), [1]);
});
case!(now, c_sc2034_reply_still_fires_under_bash, {
    assert_eq!(lines(&shell("bash", "reply=x\n"), 2034), [1]);
});

// SC2329 / SC2119 / SC2120: a function registered with zsh is used. Each script ends in
// `exit 0`, which makes an uncalled function "never invoked" (SC2329) under bash.
const NEVER_CALLED: &str = "my_hook() { echo hi; }\nexit 0\n";

case!(now, c_sc2329_control_unregistered_function_is_reported, {
    assert_eq!(lines(&zsh(NEVER_CALLED), 2329), [1]);
});

macro_rules! registered {
    ($mode:ident, $n:ident, $reg:expr) => {
        case!($mode, $n, {
            let script = format!("my_hook() {{ echo hi; }}\n{}\nexit 0\n", $reg);
            let got = zsh(&script);
            assert!(!has(&got, 2329), "SC2329 although registered ({:?}) for:\n{script}", pairs(&got));
        });
    };
}
registered!(now, c_sc2329_add_zsh_hook_precmd, "add-zsh-hook precmd my_hook");
registered!(now, c_sc2329_add_zsh_hook_preexec, "add-zsh-hook preexec my_hook");
registered!(now, c_sc2329_add_zsh_hook_chpwd, "add-zsh-hook chpwd my_hook");
registered!(now, c_sc2329_zle_dash_n_widget_same_name, "zle -N my_hook");
registered!(now, c_sc2329_zle_dash_n_widget_other_name, "zle -N my-widget my_hook");
registered!(now, c_sc2329_compdef, "compdef my_hook mycmd");
registered!(now, c_sc2329_hook_array_append, "precmd_functions+=(my_hook)");
registered!(now, c_sc2329_hook_array_append_unparenthesised, "preexec_functions+=my_hook");
registered!(now, c_sc2329_hook_array_assignment, "chpwd_functions=(my_hook $chpwd_functions)");
registered!(now, c_sc2329_trap, "trap my_hook EXIT");

case!(now, c_sc2329_hook_named_functions_are_called_by_zsh, {
    for name in ["precmd", "preexec", "chpwd", "zshexit", "TRAPINT", "TRAPUSR1"] {
        let script = format!("{name}() {{ echo hi; }}\nexit 0\n");
        let got = zsh(&script);
        assert!(!has(&got, 2329), "SC2329 for the zsh hook function {name}: {:?}", pairs(&got));
    }
});

case!(now, c_sc2329_registration_lines_do_not_change_bash, {
    let script = "my_hook() { echo hi; }\nadd-zsh-hook precmd my_hook\nexit 0\n";
    assert_eq!(lines(&shell("bash", script), 2329), [1]);
});

case!(now, c_sc2119_2120_function_registered_as_hook_gets_arguments_from_zsh, {
    let script = "my_hook() { echo \"$1\"; }\nmy_hook\nadd-zsh-hook preexec my_hook\n";
    let got = zsh(script);
    assert!(!has(&got, 2119) && !has(&got, 2120), "{:?}", pairs(&got));
});
case!(now, c_sc2119_2120_control_unregistered_function, {
    let got = zsh("my_hook() { echo \"$1\"; }\nmy_hook\n");
    assert_eq!((lines(&got, 2120), lines(&got, 2119)), (vec![1], vec![2]));
});

case!(now, c_sc2004_dollar_plus_and_dollar_hash_in_arithmetic, {
    for s in ["(( $+commands[ls] ))\n", "(( $+foo > 0 ))\n", "(( $#a > 1 ))\n"] {
        assert!(!has(&zsh(s), 2004), "SC2004 for {s:?}");
    }
});
case!(now, c_sc2004_plain_dollar_in_arithmetic_still_fires, {
    assert_eq!(lines(&zsh("x=1\n(( $x + 1 ))\n"), 2004), [2]);
});

case!(now, c_sc2030_2031_last_pipeline_element_runs_in_the_current_shell, {
    let got = zsh("echo hi | read -r v\necho \"$v\"\n");
    assert!(!has(&got, 2030) && !has(&got, 2031), "{:?}", pairs(&got));
});
case!(now, c_sc2030_2031_earlier_pipeline_element_is_still_a_subshell, {
    let got = zsh("read -r v | cat\necho \"$v\"\n");
    assert_eq!((lines(&got, 2030), lines(&got, 2031)), (vec![1], vec![2]));
});
case!(now, c_sc2030_2031_still_fire_under_bash_for_the_last_element, {
    let got = shell("bash", "echo hi | read -r v\necho \"$v\"\n");
    assert_eq!((lines(&got, 2030), lines(&got, 2031)), (vec![1], vec![2]));
});

case!(now, c_sc2221_2222_zsh_patterns_are_not_compared_as_literals, {
    for s in [
        "case $1 in\n  (a|b)*) echo ab ;;\n  (c|d)*) echo cd ;;\nesac\n",
        "case $1 in\n  <1-5>) echo low ;;\n  <6-9>) echo high ;;\nesac\n",
        "case $1 in\n  <->) echo n ;;\n  x) echo x ;;\nesac\n",
    ] {
        let got = zsh(s);
        assert!(!has(&got, 2221) && !has(&got, 2222), "SC2221/2222 for:\n{s}{:?}", pairs(&got));
    }
});
case!(now, c_sc2195_zsh_number_range_matches_a_numeric_case_word, {
    let got = zsh("case 5 in\n  <->) echo n ;;\nesac\n");
    assert!(!has(&got, 2195), "{:?}", pairs(&got));
});

case!(now, c_sc2148_zsh_file_names_need_no_shebang, {
    for name in ["plugin.zsh", "pure.zsh-theme", ".zshrc", ".zshenv", ".zprofile", ".zlogin", ".zlogout"] {
        let got = lint_named(name, &[], "echo hi\n");
        assert!(!has(&got, 2148), "SC2148 for {name}: {:?}", pairs(&got));
    }
});
case!(now, c_sc2148_still_fires_for_unnamed_shell_scripts, {
    let got = lint_named("script.txt", &[], "echo hi\n");
    assert!(has(&got, 2148), "{:?}", pairs(&got));
});

// ---- (d) off, because no zsh-true form exists --------------------------------------------

/// (code, zsh-valid repro, what zsh does with it). Each repro is `zsh -n` clean (re-checked
/// below when zsh is installed). Under zsh the code must be silent; under bash it must
/// still fire (the code is not removed, only gated).
const CLASS_D: &[(u32, &str, &str)] = &[
    (2296, "echo \"${(L)1}\"\n", "`(L)` is a parameter expansion flag"),
    (2298, "echo \"${${1}%y}\"\n", "nested expansion, applies `%y` to the inner result"),
    (2299, "echo \"${${1%a}%b}\"\n", "nested expansions are valid"),
    (2300, "echo \"${$(date)#a}\"\n", "expansion applied to a command substitution is valid"),
    (2082, "echo \"${${1}}\"\n", "`${${x}}` is the nested form"),
    (2190, "typeset -A m\nm=(a 1 b 2)\necho \"${m[a]}\"\n", "flat key/value list is native"),
    (2051, "for i in {1..$1}; do echo \"$i\"; done\n", "variables in brace ranges expand"),
    (2277, "0=foo\n", "`$0` is assignable"),
    (2270, "1=foo\n", "positional parameters are assignable"),
    (2168, "local x=1\necho \"$x\"\n", "`local` is allowed at top level"),
    (2180, "m[1][2]=3\n", "spec class (d): the text is bash-specific (zsh has no multidimensional arrays either)"),
];

case!(now, d_every_repro_is_valid_zsh_and_silent_under_zsh_for_the_ones_already_gated, {
    if !have_zsh() {
        return;
    }
    for (code, s, _) in CLASS_D {
        assert!(zsh_n_ok(s), "zsh -n rejects the class (d) repro for SC{code}:\n{s}");
    }
});

macro_rules! class_d {
    ($mode:ident, $n:ident, $idx:expr) => {
        case!($mode, $n, {
            let (code, s, why) = CLASS_D[$idx];
            let z = zsh(s);
            assert!(!has(&z, code), "SC{code} under zsh ({why}); findings {:?} for:\n{s}", pairs(&z));
        });
    };
}
class_d!(now, d_sc2296_flags, 0);
class_d!(now, d_sc2298_nested_with_modifier, 1);
class_d!(now, d_sc2299_nested_expansion, 2);
class_d!(now, d_sc2300_command_substitution_target, 3);
class_d!(now, d_sc2082_nested_braces, 4);
class_d!(now, d_sc2190_flat_associative_array_init, 5);
class_d!(now, d_sc2051_variable_in_brace_range, 6);
class_d!(now, d_sc2277_assign_to_zero, 7);
class_d!(now, d_sc2270_assign_positional, 8);
class_d!(now, d_sc2168_local_at_top_level, 9);
class_d!(now, d_sc2180_multi_dimensional_subscript, 10);

// Extrapolation beyond the design's class (d) list (which names SC2277 and SC2270 only):
// zsh really assigns `$0`, so the generic "can't be assigned" wording of SC2278-SC2280 is
// false too. Flagged in the report; drop this test if the design decides otherwise.
case!(now, d_assigning_zero_gets_no_replacement_code_either, {
    let got = zsh("0=foo\n");
    let other: Vec<u32> = codes(&got).into_iter().filter(|c| (2270..=2280).contains(c)).collect();
    assert!(other.is_empty(), "an assignment to $0 is valid in zsh, got SC{other:?}");
});

case!(now, d_sc2338_optional_check_is_silent_under_zsh, {
    let got = zsh_with(&["--enable=require-double-equals"], "[[ $1 = $2 ]]\n");
    assert!(!has(&got, 2338), "{:?}", pairs(&got));
});
case!(now, d_sc2338_optional_check_still_fires_under_bash, {
    let got = lint(&["--shell=bash", "--enable=require-double-equals"], "[[ $1 = $2 ]]\n");
    assert_eq!(lines(&got, 2338), [1]);
});

case!(now, d_every_class_d_code_still_fires_under_bash, {
    for (code, s, _) in CLASS_D {
        let b = shell("bash", s);
        assert!(has(&b, *code), "SC{code} no longer fires under bash for:\n{s}\nfindings {:?}", pairs(&b));
    }
});
