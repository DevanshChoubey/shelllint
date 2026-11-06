// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSpacefulnessCfg`, lines 2298-2298 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_counting_reference, is_param_to, is_quote_free, is_quoted_alternative_reference, used_as_command_name};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_analyzer::cfg_analysis::{CFVariableProp, SpaceStatus, VariableState, get_incoming_state};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, get_braced_reference, get_path, is_array_expansion, oversimplify};
use shellcheck_core::data::SPECIAL_VARIABLES_WITHOUT_SPACES;

use crate::support::surround_with;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSpacefulnessCfg",
    registry: "node",
    order: 96,
    optional: None,
    kind: CheckKind::Node(check_spacefulness_cfg),
};

fn check_spacefulness_cfg(params: &Params<'_>, t: Id, c: &mut Comments) {
    check_spacefulness_cfg_prime(true, params, t, c)
}

// Don't suggest quotes if this will instead be autocorrected
// from $foo=bar to foo=bar. This is not pretty but ok.
fn quotes_may_conflict_with_sc2281(params: &Params<'_>, t: Id) -> bool {
    let path = get_path(params.ast, t);
    let [_, w, sc, ..] = &path[..] else { return false };
    let Token::T_NormalWord(parts) = params.ast.get(*w) else { return false };
    let [me, lit, ..] = &parts[..] else { return false };
    let Token::T_Literal(s) = params.ast.get(*lit) else { return false };
    if !s.starts_with('=') {
        return false;
    }
    let Token::T_SimpleCommand(_, words) = params.ast.get(*sc) else { return false };
    let Some(cmd) = words.first() else { return false };
    t == *me && *w == *cmd
}

fn add_double_quotes_around(params: &Params<'_>, token: Id) -> shellcheck_core::interface::Fix {
    surround_with(token, params, "\"")
}

fn check_spacefulness_cfg_prime(dirty_pass: bool, params: &Params<'_>, token: Id, c: &mut Comments) {
    let Token::T_DollarBraced(_, list) = params.ast.get(token) else { return };
    let id = token;
    let braced_string = oversimplify(params.ast, *list).concat();
    let name = get_braced_reference(&braced_string);

    let needs_quoting = !is_array_expansion(params.ast, token) // There's another warning for this
        && !is_counting_reference(params.ast, token)
        && !is_quote_free(params.ast, params.shell_type, token)
        && !is_quoted_alternative_reference(params.ast, token)
        && !used_as_command_name(params.ast, token);

    let is_clean_state = |state: &VariableState| {
        state.variable_properties.all_have(CFVariableProp::CFVPInteger)
            || state.variable_value.space_status == SpaceStatus::SpaceStatusClean
    };

    let is_clean = (|| {
        let cfga = params.cfg_analysis.as_deref()?;
        let state = get_incoming_state(cfga, id)?;
        let value = state.variable_in_scope(&name)?;
        Some(is_clean_state(&value))
    })()
    .unwrap_or(false);

    let is_default_assignment = |token: Id| {
        let modifier = get_braced_modifier(&braced_string);
        ["=", ":="].iter().any(|p| modifier.starts_with(p)) && is_param_to(params.ast, ":", token)
    };

    if needs_quoting && (dirty_pass == !is_clean) {
        if !(SPECIAL_VARIABLES_WITHOUT_SPACES.contains(&name.as_str())
            || quotes_may_conflict_with_sc2281(params, token))
        {
            if dirty_pass {
                if is_default_assignment(token) {
                    c.info(token, 2223, "This default assignment may cause DoS due to globbing. Quote it.");
                } else {
                    c.info_with_fix(
                        id,
                        2086,
                        "Double quote to prevent globbing and word splitting.",
                        add_double_quotes_around(params, token),
                    );
                }
            } else {
                c.style_with_fix(
                    id,
                    2248,
                    "Prefer double quoting even when variables don't contain special characters.",
                    add_double_quotes_around(params, token),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn codes(script: &str) -> Vec<u32> {
        run_check("Analytics.hs", "checkSpacefulnessCfg", "node", script)
            .expect("check is registered")
            .into_iter()
            .map(|c| c as u32)
            .collect()
    }

    #[test]
    fn prop_check_spacefulness_cfg1() {
        assert!(!codes("a='cow moo'; echo $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg2() {
        assert!(codes("a='cow moo'; [[ $a ]]").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg3() {
        assert!(codes("a='cow*.mp3'; echo \"$a\"").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg4() {
        assert!(!codes("for f in *.mp3; do echo $f; done").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg4a() {
        assert!(codes("foo=3; foo=$(echo $foo)").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg5() {
        assert!(!codes("a='*'; b=$a; c=lol${b//foo/bar}; echo $c").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg6() {
        assert!(!codes("a=foo$(lol); echo $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg7() {
        assert!(!codes("a=foo\\ bar; rm $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg8() {
        assert!(codes("a=foo\\ bar; a=foo; rm $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg10() {
        assert!(!codes("rm $1").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg11() {
        assert!(!codes("rm ${10//foo/bar}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg12() {
        assert!(codes("(( $1 + 3 ))").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg13() {
        assert!(codes("if [[ $2 -gt 14 ]]; then true; fi").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg14() {
        assert!(codes("foo=$3 env").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg15() {
        assert!(codes("local foo=$1").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg16() {
        assert!(codes("declare foo=$1").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg17() {
        assert!(!codes("echo foo=$1").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg18() {
        assert!(codes("$1 --flags").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg19() {
        assert!(!codes("echo $PWD").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg20() {
        assert!(codes("n+='foo bar'").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg21() {
        assert!(codes("select foo in $bar; do true; done").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg22() {
        assert!(codes("echo $\"$1\"").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg23() {
        assert!(codes("a=(1); echo ${a[@]}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg24() {
        assert!(!codes("a='a    b'; cat <<< $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg25() {
        assert!(!codes("a='s/[0-9]//g'; sed $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg26() {
        assert!(!codes("a='foo bar'; echo {1,2,$a}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg27() {
        assert!(codes("echo ${a:+'foo'}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg28() {
        assert!(codes("exec {n}>&1; echo $n").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg29() {
        assert!(codes("n=$(stuff); exec {n}>&-;").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg30() {
        assert!(!codes("file='foo bar'; echo foo > $file;").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg31() {
        assert!(codes("echo \"`echo \\\"$1\\\"`\"").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg32() {
        assert!(codes("var=$1; [ -v var ]").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg33() {
        assert!(!codes("for file; do echo $file; done").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg34() {
        assert!(!codes("declare foo$n=$1").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg35() {
        assert!(codes("echo ${1+\"$1\"}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg36() {
        assert!(codes("arg=$#; echo $arg").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg37() {
        assert!(codes("@test 'status' {\n [ $status -eq 0 ]\n}").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg38() {
        assert!(!codes("a=; echo $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg39() {
        assert!(codes("a=''\"\"''; b=x$a; echo $b").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg40() {
        assert!(codes("a=$((x+1)); echo $a").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg41() {
        assert!(codes("exec $1 --flags").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg42() {
        assert!(codes("run $1 --flags").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg43() {
        assert!(codes("$foo=42").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg44() {
        assert!(!codes("#!/bin/sh\nexport var=$value").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg45() {
        assert!(codes("wait -zzx -p foo; echo $foo").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg46() {
        assert!(codes("x=0; (( x += 1 )); echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg47() {
        assert!(codes("x=0; (( x-- )); echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg48() {
        assert!(codes("x=0; (( ++x )); echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg49() {
        assert!(codes("for i in 1 2 3; do echo $i; done").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg50() {
        assert!(!codes("for i in 1 2 *; do echo $i; done").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg51() {
        assert!(!codes("x='foo bar'; x && x=1; echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg52() {
        assert!(codes("x=1; if f; then x='foo bar'; exit; fi; echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg53() {
        assert!(codes("s=1; f() { local s='a b'; }; f; echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg54() {
        assert!(codes("s='a b'; f() { s=1; }; f; echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg55() {
        assert!(!codes("s='a b'; x && f() { s=1; }; f; echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg56() {
        assert!(codes("s=1; cat <(s='a b'); echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg57() {
        assert!(codes("declare -i s=0; s=$(f); echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg58() {
        assert!(!codes("f() { declare -i s; }; f; s=$(var); echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg59() {
        assert!(codes("f() { declare -gi s; }; f; s=$(var); echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg60() {
        assert!(!codes("declare -i s; declare +i s; s=$(foo); echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg61() {
        assert!(!codes("declare -x X; y=foo$X; echo $y;").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg62() {
        assert!(codes("f() { declare -x X; y=foo$X; echo $y; }").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg63() {
        assert!(!codes("f && declare -i s; s='x + y'; echo $s").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg64() {
        assert!(codes("declare -i s; s='x + y'; x=$s; echo $x").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg65() {
        assert!(codes("f() { s=$?; echo $s; }; f").is_empty());
    }

    #[test]
    fn prop_check_spacefulness_cfg66() {
        assert!(codes("f() { s=$?; echo $s; }").is_empty());
    }
}
