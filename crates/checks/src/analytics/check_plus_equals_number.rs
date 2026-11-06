// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPlusEqualsNumber`, lines 5376-5406 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_analyzer::cfg_analysis::{
    ProgramState, get_incoming_state, variable_may_be_assigned_integer,
    variable_may_be_declared_integer,
};
use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::astlib::{get_unmodified_parameter_expansion, get_unquoted_literal};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPlusEqualsNumber",
    registry: "node",
    order: 100,
    optional: None,
    kind: CheckKind::Node(check_plus_equals_number),
};

fn check_plus_equals_number(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(AssignmentMode::Append, var, _, word) = p.ast.get(t) else { return };
    let word = *word;
    let Some(cfga) = p.cfg_analysis.as_deref() else { return };
    let Some(state) = get_incoming_state(cfga, t) else { return };
    if !is_number(p, state, word) {
        return;
    }
    if variable_may_be_declared_integer(state, var).unwrap_or(false) {
        return;
    }
    // Recommend "typeset" because ksh does not have "declare".
    c.warn(
        t,
        2324,
        "var+=1 will append, not increment. Use (( var += 1 )), typeset -i var, or quote number to silence.",
    );
}

fn is_number(p: &Params<'_>, state: &ProgramState, word: Id) -> bool {
    let unquoted_literal = get_unquoted_literal(p.ast, word);
    let is_empty = unquoted_literal.as_deref() == Some("");
    let is_unquoted_number = !is_empty
        && unquoted_literal.as_deref().is_some_and(|s| s.chars().all(|ch| ch.is_ascii_digit()));
    let is_numerical_variable_name = unquoted_literal
        .as_deref()
        .and_then(|s| variable_may_be_assigned_integer(state, s))
        .unwrap_or(false);
    let is_numerical_variable_expansion = match p.ast.get(word) {
        Token::T_NormalWord(parts) if parts.len() == 1 => get_unmodified_parameter_expansion(p.ast, parts[0])
            .and_then(|s| variable_may_be_assigned_integer(state, &s))
            .unwrap_or(false),
        _ => false,
    };
    is_unquoted_number || is_numerical_variable_name || is_numerical_variable_expansion
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn fires(script: &str) -> bool {
        let codes = run_check(crate::CHECKS, "Analytics.hs", "checkPlusEqualsNumber", "node", script).unwrap();
        codes.contains(&2324)
    }

    #[test]
    fn prop_check_plus_equals_number1() {
        assert!(fires("x+=1"));
    }
    #[test]
    fn prop_check_plus_equals_number2() {
        assert!(fires("x+=42"));
    }
    #[test]
    fn prop_check_plus_equals_number3() {
        assert!(!fires("(( x += 1 ))"));
    }
    #[test]
    fn prop_check_plus_equals_number4() {
        assert!(!fires("declare -i x=0; x+=1"));
    }
    #[test]
    fn prop_check_plus_equals_number5() {
        assert!(!fires("x+='1'"));
    }
    #[test]
    fn prop_check_plus_equals_number6() {
        assert!(!fires("n=foo; x+=n"));
    }
    #[test]
    fn prop_check_plus_equals_number7() {
        assert!(fires("n=4; x+=n"));
    }
    #[test]
    fn prop_check_plus_equals_number8() {
        assert!(fires("n=4; x+=$n"));
    }
    #[test]
    fn prop_check_plus_equals_number9() {
        assert!(!fires("declare -ia var; var[x]+=1"));
    }
}
