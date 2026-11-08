// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkOverwrittenExitCode`, lines 5281-5317 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_analyzer::cfg_analysis::{CfgAnalysis, does_post_dominate, get_incoming_state};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_command_basename, get_command_name, get_literal_string};
use std::collections::BTreeSet;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkOverwrittenExitCode",
    registry: "node",
    order: 97,
    optional: None,
    kind: CheckKind::Node(check_overwritten_exit_code),
};

fn check_overwritten_exit_code(params: &Params<'_>, t: Id, c: &mut Comments) {
    if let &Token::T_DollarBraced(_, val) = params.ast.get(t) {
        if get_literal_string(params.ast, val).as_deref() == Some("?") {
            check(params, t, t, c);
        }
    }
}

// `check id` in the Haskell `where` clause; `id` is `getId t`, which is `t`.
fn check(params: &Params<'_>, t: Id, id: Id, c: &mut Comments) {
    let Some(cfga) = params.cfg_analysis.as_deref() else { return };
    let Some(state) = get_incoming_state(cfga, id) else { return };
    let exit_code_ids = state.exit_codes();
    if exit_code_ids.is_empty() {
        return;
    }

    // `traverse (Map.lookup k idMap)`: every id is in the arena.
    let exit_code_tokens: Vec<Id> = exit_code_ids.iter().copied().collect();
    if exit_code_tokens.iter().all(|&e| is_condition(params, e)) && !used_unconditionally(cfga, t, exit_code_ids) {
        c.warn(
            id,
            2319,
            "This $? refers to a condition, not a command. Assign to a variable to avoid it being overwritten.",
        );
    }
    if exit_code_tokens.iter().all(|&e| is_printing(params, e)) {
        c.warn(
            id,
            2320,
            "This $? refers to echo/printf, not a previous command. Assign to variable to avoid it being overwritten.",
        );
    }
}

fn is_condition(params: &Params<'_>, t: Id) -> bool {
    match params.ast.get(t) {
        Token::T_Condition(..) => true,
        Token::T_SimpleCommand(..) => get_command_name(params.ast, t).as_deref() == Some("test"),
        _ => false,
    }
}

// If we don't do anything based on the condition, assume we wanted the condition itself
// This helps differentiate `x; [ $? -gt 0 ] && exit $?` vs `[ cond ]; exit $?`
fn used_unconditionally(cfga: &CfgAnalysis, t: Id, test_ids: &BTreeSet<Id>) -> bool {
    test_ids.iter().all(|&c| does_post_dominate(cfga, t, c))
}

fn is_printing(params: &Params<'_>, t: Id) -> bool {
    matches!(get_command_basename(params.ast, t).as_deref(), Some("echo") | Some("printf"))
}

#[cfg(test)]
mod tests {
    use shelllint_analyzer::testing::run_check;

    fn codes(s: &str) -> Vec<u32> {
        run_check(crate::CHECKS, "Analytics.hs", "checkOverwrittenExitCode", "node", s)
            .unwrap()
            .into_iter()
            .map(|c| c as u32)
            .collect()
    }

    #[test]
    fn prop_check_overwritten_exit_code1() {
        assert_eq!(codes("x; [ $? -eq 1 ] || [ $? -eq 2 ]"), [2319]);
    }
    #[test]
    fn prop_check_overwritten_exit_code2() {
        assert!(codes("x; [ $? -eq 1 ]").is_empty());
    }
    #[test]
    fn prop_check_overwritten_exit_code3() {
        assert_eq!(codes("x; echo \"Exit is $?\"; [ $? -eq 0 ]"), [2320]);
    }
    #[test]
    fn prop_check_overwritten_exit_code4() {
        assert!(codes("x; [ $? -eq 0 ] && echo Success").is_empty());
    }
    #[test]
    fn prop_check_overwritten_exit_code5() {
        assert_eq!(codes("x; if [ $? -eq 0 ]; then var=$?; fi"), [2319]);
    }
    #[test]
    fn prop_check_overwritten_exit_code6() {
        assert_eq!(codes("x; [ $? -gt 0 ] && fail=$?"), [2319]);
    }
    #[test]
    fn prop_check_overwritten_exit_code7() {
        assert!(codes("[ 1 -eq 2 ]; status=$?").is_empty());
    }
    #[test]
    fn prop_check_overwritten_exit_code8() {
        assert!(codes("[ 1 -eq 2 ]; exit $?").is_empty());
    }
}
