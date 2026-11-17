// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests for the zsh work (no ShellCheck counterpart).
//
// Test layer 4, class (b): codes that are only true in zsh when an option makes them
// true, plus the explicit `${=x}` (split) and `${~x}` (glob) forms. For each code:
//   * `*_silent_by_default`   zsh defaults (no splitting, no globbing, 1-based arrays)
//   * `*_explicit_*`          `${=x}` / `${~x}` fire whatever the options are
//   * `*_option_on`           the option on fires (pending OT: until the option tracker
//                             lands `zsh_option` answers zsh defaults, so these are ignored)
//   * `*_other_condition_*`   the check's own non-splitting conditions stay silent
//   * `*_bash_unchanged`      the same snippet under bash is exactly what main printed
//
// zsh evidence for the claims (zsh 5.9, `/usr/bin/zsh -fc`):
//   x="a b"; for i in $x; do echo "[$i]"; done            -> [a b]
//   setopt shwordsplit; (same)                            -> [a] [b]
//   x="a b"; for i in ${=x}; do echo "[$i]"; done         -> [a] [b]
//   set -- "a b" c; for x in $*; do echo "[$x]"; done     -> [a b] [c]   (same for $@)
//   setopt shwordsplit; (same)                            -> [a] [b] [c]
//   p="a*"; [[ abc == $p ]]                               -> no match;  with globsubst or ${~p}: match
//   p="a*"; x=abcd; echo ${x#$p}                          -> abcd;      with globsubst or ${x#${~p}}: bcd
//   p="a*"; case abc in $p) ...                           -> no match;  with globsubst or ${~p}: match
//   a=(1 2 3); echo $a                                    -> 1 2 3;     with ksharrays: 1
//   a=(1 2); a=str; typeset -p a                          -> typeset a=str;   with ksharrays: a=( str 2 )
//   echo "a\nb"                                           -> two lines; with bsdecho: a\nb
#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::*;

fn has(codes: &[u32], c: u32) -> bool {
    codes.contains(&c)
}

// ---------------------------------------------------------------- SC2086

#[test]
fn sc2086_silent_by_default() {
    assert_eq!(zsh_codes("x=\"a b\"\necho $x\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("x=\"a b\"\nrm -- $x $x\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("f() { cp $1 $2; }\nf a b\n"), Vec::<u32>::new());
}

#[test]
fn sc2086_explicit_split_fires_without_any_option() {
    assert!(has(&zsh_codes("x=\"a b\"\ncp ${=x} y\n"), 2086));
}

#[test]
fn sc2086_explicit_glob_fires_without_any_option() {
    assert!(has(&zsh_codes("x=\"a*\"\necho ${~x}\n"), 2086));
}

#[test]
fn sc2086_explicit_flag_forms_fire() {
    // `${==x}` (split with the flag doubled) and `${~~x}` are still the explicit forms.
    assert!(has(&zsh_codes("x=\"a b\"\ncp ${==x} y\n"), 2086));
    assert!(has(&zsh_codes("x=\"a b\"\necho ${~~x}\n"), 2086));
}

#[test]
fn sc2086_explicit_form_in_a_function_and_pipeline_fires() {
    assert!(has(&zsh_codes("f() { echo hi | cp ${=1} y; }\n"), 2086));
}

#[test]
fn sc2086_other_condition_quoted_and_assignment_and_double_bracket_stay_silent() {
    // Quoted, an assignment and `[[ ]]` never split, whatever the expansion looks like.
    assert!(!has(&zsh_codes("x=\"a b\"\necho \"$x\"\n"), 2086));
    assert!(!has(&zsh_codes("x=\"a b\"\ny=${=x}\necho \"$y\"\n"), 2086));
    assert!(!has(&zsh_codes("x=\"a b\"\n[[ ${=x} ]]\n"), 2086));
}

#[test]
fn sc2086_bash_unchanged() {
    assert_same_as_bash_today("x=\"a b\"\necho $x\n", &[2086]);
    assert_same_as_bash_today("x=\"a b\"\ncp ${=x} y\n", &[2034, 2086]);
    assert_same_as_bash_today("x=\"a b\"\ny=$x\necho \"$y\"\n", &[]);
}

#[test]
fn sc2086_shwordsplit_on_fires() {
    assert!(has(&zsh_codes_opts("x=\"a b\"\necho $x\n", &["shwordsplit"]), 2086));
    assert!(has(&zsh_codes_opts("x=\"a b\"\necho $x\n", &["SH_WORD_SPLIT"]), 2086));
}

#[test]
fn sc2086_globsubst_on_fires() {
    assert!(has(&zsh_codes_opts("x=\"a*\"\necho $x\n", &["globsubst"]), 2086));
}

#[test]
fn sc2086_option_off_again_is_silent() {
    assert!(!has(&zsh_codes_opts("x=\"a b\"\necho $x\n", &["shwordsplit", "noshwordsplit"]), 2086));
}

#[test]
fn sc2086_option_on_other_conditions_stay_silent() {
    for script in [
        "x=\"a b\"\necho \"$x\"\n",
        "x=\"a b\"\ny=$x\necho \"$y\"\n",
        "x=\"a b\"\n[[ $x ]]\n",
        "x=abc\necho $x\n",
        "for f in $@; do echo \"$f\"; done\n",
    ] {
        assert!(!has(&zsh_codes_opts(script, &["shwordsplit"]), 2086), "{script}");
    }
}

// ---------------------------------------------------------------- SC2223

#[test]
fn sc2223_silent_by_default() {
    assert_eq!(zsh_codes(": ${x:=default}\n"), Vec::<u32>::new());
}

#[test]
fn sc2223_bash_unchanged() {
    assert_same_as_bash_today(": ${x:=default}\n", &[2223]);
}

#[test]
fn sc2223_other_condition_quoted_default_stays_silent_in_bash_and_zsh() {
    assert_same_as_bash_today(": \"${x:=default}\"\n", &[]);
    assert_eq!(zsh_codes(": \"${x:=default}\"\n"), Vec::<u32>::new());
}

#[test]
fn sc2223_option_on_fires() {
    assert!(has(&zsh_codes_opts(": ${x:=default}\n", &["shwordsplit"]), 2223));
    assert!(has(&zsh_codes_opts(": ${x:=default}\n", &["globsubst"]), 2223));
}

// ---------------------------------------------------------------- SC2048

#[test]
fn sc2048_silent_by_default() {
    assert_eq!(zsh_codes("rm $*\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("for f in $*; do echo \"$f\"; done\n"), Vec::<u32>::new());
}

#[test]
fn sc2048_explicit_split_fires() {
    assert!(has(&zsh_codes("rm ${=*}\n"), 2048));
}

#[test]
fn sc2048_other_condition_quoted_stays_silent() {
    assert!(!has(&zsh_codes("rm \"$*\"\n"), 2048));
    assert!(!has(&zsh_codes("rm \"${=*}\"\n"), 2048));
}

#[test]
fn sc2048_bash_unchanged() {
    assert_same_as_bash_today("rm $*\n", &[2048, 2086]);
}

#[test]
fn sc2048_shwordsplit_on_fires() {
    assert!(has(&zsh_codes_opts("rm $*\n", &["shwordsplit"]), 2048));
}

#[test]
fn sc2048_option_on_quoted_stays_silent() {
    assert!(!has(&zsh_codes_opts("rm \"$*\"\n", &["shwordsplit"]), 2048));
}

// ---------------------------------------------------------------- SC2068

#[test]
fn sc2068_silent_by_default() {
    assert_eq!(zsh_codes("rm $@\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("a=(1 2)\nrm ${a[@]}\n"), Vec::<u32>::new());
}

#[test]
fn sc2068_explicit_split_fires() {
    assert!(has(&zsh_codes("a=(1 2)\nrm ${=a[@]}\n"), 2068));
}

#[test]
fn sc2068_other_condition_quoted_stays_silent() {
    assert!(!has(&zsh_codes("a=(1 2)\nrm \"${a[@]}\"\n"), 2068));
    assert!(!has(&zsh_codes("rm \"$@\"\n"), 2068));
}

#[test]
fn sc2068_bash_unchanged() {
    assert_same_as_bash_today("rm $@\n", &[2068]);
}

#[test]
fn sc2068_shwordsplit_on_fires() {
    assert!(has(&zsh_codes_opts("rm $@\n", &["shwordsplit"]), 2068));
    assert!(has(&zsh_codes_opts("a=(1 2)\nrm ${a[@]}\n", &["shwordsplit"]), 2068));
}

// ---------------------------------------------------------------- SC2053

#[test]
fn sc2053_silent_by_default() {
    assert_eq!(zsh_codes("a=x\nb=y\n[[ $a == $b ]]\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("a=x\nb=y\n[[ $a != $b ]]\n"), Vec::<u32>::new());
}

#[test]
fn sc2053_explicit_glob_fires() {
    assert!(has(&zsh_codes("a=x\nb=y\n[[ $a == ${~b} ]]\n"), 2053));
}

#[test]
fn sc2053_other_condition_quoted_and_literal_stay_silent() {
    assert!(!has(&zsh_codes("a=x\nb=y\n[[ $a == \"$b\" ]]\n"), 2053));
    assert!(!has(&zsh_codes("a=x\n[[ $a == b ]]\n"), 2053));
    assert!(!has(&zsh_codes("a=x\nb=y\n[[ $a = ${~b}z ]]\n"), 2053), "only a lone expansion is reported");
}

#[test]
fn sc2053_bash_unchanged() {
    assert_same_as_bash_today("a=x\nb=y\n[[ $a == $b ]]\n", &[2053]);
}

#[test]
fn sc2053_globsubst_on_fires() {
    assert!(has(&zsh_codes_opts("a=x\nb=y\n[[ $a == $b ]]\n", &["globsubst"]), 2053));
}

#[test]
fn sc2053_shwordsplit_alone_does_not_make_it_true() {
    assert!(!has(&zsh_codes_opts("a=x\nb=y\n[[ $a == $b ]]\n", &["shwordsplit"]), 2053));
}

// ---------------------------------------------------------------- SC2295

#[test]
fn sc2295_silent_by_default() {
    assert_eq!(zsh_codes("p='a*'\nx=abcd\necho \"${x#$p}\"\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("x=abcd\necho \"${x%$(date)}\"\n"), Vec::<u32>::new());
}

#[test]
fn sc2295_explicit_glob_fires() {
    assert!(has(&zsh_codes("p='a*'\nx=abcd\necho \"${x#${~p}}\"\n"), 2295));
}

#[test]
fn sc2295_other_condition_quoted_stays_silent() {
    assert!(!has(&zsh_codes("p='a*'\nx=abcd\necho \"${x#\"$p\"}\"\n"), 2295));
}

#[test]
fn sc2295_bash_unchanged() {
    assert_same_as_bash_today("p='a*'\nx=abcd\necho \"${x#$p}\"\n", &[2295]);
}

#[test]
fn sc2295_globsubst_on_fires() {
    assert!(has(&zsh_codes_opts("p='a*'\nx=abcd\necho \"${x#$p}\"\n", &["globsubst"]), 2295));
    assert!(has(&zsh_codes_opts("x=abcd\necho \"${x%$(date)}\"\n", &["globsubst"]), 2295));
}

// ---------------------------------------------------------------- SC2254

#[test]
fn sc2254_silent_by_default() {
    assert_eq!(zsh_codes("p='a*'\ncase $1 in $p) echo hit;; esac\n"), Vec::<u32>::new());
}

#[test]
fn sc2254_explicit_glob_fires() {
    assert!(has(&zsh_codes("p='a*'\ncase $1 in ${~p}) echo hit;; esac\n"), 2254));
}

#[test]
fn sc2254_other_condition_quoted_and_already_a_glob_stay_silent() {
    assert!(!has(&zsh_codes("p='a*'\ncase $1 in \"$p\") echo hit;; esac\n"), 2254));
    assert!(!has(&zsh_codes("p='a'\ncase $1 in ${~p}*) echo hit;; esac\n"), 2254), "already a glob");
}

#[test]
fn sc2254_bash_unchanged() {
    assert_same_as_bash_today("p='a*'\ncase $1 in $p) echo hit;; esac\n", &[2254]);
}

#[test]
fn sc2254_globsubst_on_fires() {
    assert!(has(&zsh_codes_opts("p='a*'\ncase $1 in $p) echo hit;; esac\n", &["globsubst"]), 2254));
}

// ---------------------------------------------------------------- SC2128

#[test]
fn sc2128_silent_by_default() {
    assert_eq!(zsh_codes("a=(1 2)\necho $a\n"), Vec::<u32>::new());
    assert_eq!(zsh_codes("a=(1 2)\necho \"$a\"\n"), Vec::<u32>::new());
}

#[test]
fn sc2128_other_condition_indexed_stays_silent() {
    assert!(!has(&zsh_codes("a=(1 2)\necho \"${a[1]}\" \"${a[@]}\"\n"), 2128));
}

#[test]
fn sc2128_bash_unchanged() {
    assert_same_as_bash_today("a=(1 2)\necho $a\n", &[2128, 2086]);
    assert_same_as_bash_today("a=(1 2)\necho \"$a\"\n", &[2128]);
}

#[test]
fn sc2128_ksharrays_on_fires() {
    assert!(has(&zsh_codes_opts("a=(1 2)\necho \"$a\"\n", &["ksharrays"]), 2128));
    assert!(has(&zsh_codes_opts("a=(1 2)\necho \"$a\"\n", &["KSH_ARRAYS"]), 2128));
}

#[test]
fn sc2128_ksharrays_on_indexed_stays_silent() {
    assert!(!has(&zsh_codes_opts("a=(1 2)\necho \"${a[0]}\" \"${a[@]}\"\n", &["ksharrays"]), 2128));
}

// ---------------------------------------------------------------- SC2178

#[test]
fn sc2178_silent_by_default() {
    // zsh turns `a` into a plain string (`typeset a=str`).
    assert!(!has(&zsh_codes("a=(1 2)\na=str\necho \"${a}\"\n"), 2178));
}

#[test]
fn sc2178_other_condition_array_assignment_stays_silent() {
    assert!(!has(&zsh_codes("a=(1 2)\na=(3 4)\necho \"${a[@]}\"\n"), 2178));
}

#[test]
fn sc2178_bash_unchanged() {
    assert_same_as_bash_today("a=(1 2)\na=str\n", &[2034, 2178]);
}

#[test]
fn sc2178_ksharrays_on_fires() {
    assert!(has(&zsh_codes_opts("a=(1 2)\na=str\necho \"${a}\"\n", &["ksharrays"]), 2178));
}

// ---------------------------------------------------------------- SC2028

#[test]
fn sc2028_silent_by_default() {
    // zsh's echo expands the escapes itself.
    assert_eq!(zsh_codes("echo \"a\\nb\"\n"), Vec::<u32>::new());
}

#[test]
fn sc2028_other_condition_echo_e_stays_silent() {
    assert!(!has(&zsh_codes("echo -e \"a\\nb\"\n"), 2028));
}

#[test]
fn sc2028_bash_unchanged() {
    assert_same_as_bash_today("echo \"a\\nb\"\n", &[2028]);
    assert_same_as_bash_today("echo -e \"a\\nb\"\n", &[]);
}

#[test]
fn sc2028_bsdecho_on_fires() {
    assert!(has(&zsh_codes_opts("echo \"a\\nb\"\n", &["bsdecho"]), 2028));
    assert!(has(&zsh_codes_opts("echo \"a\\nb\"\n", &["BSD_ECHO"]), 2028));
}

#[test]
fn sc2028_bsdecho_on_echo_e_stays_silent() {
    assert!(!has(&zsh_codes_opts("echo -e \"a\\nb\"\n", &["bsdecho"]), 2028));
}

// ---------------------------------------------------------------- SC1087

#[test]
fn sc1087_silent_by_default() {
    // `$a[1]` is a subscript in zsh. (The parser lanes make the rest of the line parse.)
    assert!(!has(&zsh_codes("echo \"$a[1]\"\n"), 1087));
    assert!(!has(&zsh_codes("echo $a[1]\n"), 1087));
}

#[test]
fn sc1087_other_condition_braced_stays_silent() {
    assert!(!has(&zsh_codes("echo \"${a[1]}\"\n"), 1087));
}

#[test]
fn sc1087_bash_unchanged() {
    assert!(has(&bash_codes("echo \"$a[1]\"\n"), 1087));
    assert_same_as_bash_today("echo \"${a[1]}\"\n", &[2154]);
}

#[test]
fn sc1087_ksharrays_on_fires() {
    assert!(has(&zsh_codes_opts("a=(1 2)\necho \"$a[1]\"\n", &["ksharrays"]), 1087));
}
