// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUseBeforeDefinition`, lines 4034-4065 (e11f964)

use std::cell::RefCell;
use std::collections::HashMap;

use shelllint_analyzer::analyzerlib::{Comments, Params, parents};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_analyzer::cfg_analysis::does_post_dominate;
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_literal_string;
use shelllint_core::interface::TokenComment;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUseBeforeDefinition",
    registry: "tree",
    order: 11,
    optional: None,
    kind: CheckKind::Tree(check_use_before_definition),
};

fn check_use_before_definition(params: &Params<'_>, t: Id) -> Vec<TokenComment> {
    let Some(cfga) = params.cfg_analysis.as_deref() else {
        return vec![];
    };
    let ast = params.ast;

    let is_bats_test = |id: Id| matches!(ast.get(id), Token::T_BatsTest(..));
    let is_in_bats_test = |id: Id| parents(params, id).into_iter().any(is_bats_test);

    // `execState (doAnalysis findFunction t) Map.empty`
    let funcs: RefCell<HashMap<String, Vec<(Id, bool)>>> = RefCell::new(HashMap::new());
    ast.analyze(
        t,
        &mut |id| {
            if let Token::T_Function(_, _, name, _) = ast.get(id) {
                // Map.insertWith (++) name [new]: new definitions go first.
                funcs.borrow_mut().entry(name.clone()).or_default().insert(0, (id, is_in_bats_test(id)));
            }
        },
        &mut |_| {},
    );
    let funcs = funcs.into_inner();
    // Green cut: no point enumerating commands if there are no functions.
    if funcs.is_empty() {
        return vec![];
    }

    let mut c = Comments::new();
    ast.analyze(
        t,
        &mut |id| {
            let Token::T_SimpleCommand(_, words) = ast.get(id) else { return };
            let Some(&cmd) = words.first() else { return };
            let Some(name) = get_literal_string(ast, cmd) else { return };
            let Some(definitions) = funcs.get(&name) else { return };
            let invocations: Vec<Id> = definitions.iter().map(|d| d.0).collect();
            // Bats loads the whole file before running a test, so functions
            // defined outside of tests are available in all of them.
            if is_in_bats_test(id) && definitions.iter().any(|d| !d.1) {
                return;
            }
            // Is the function definitely being defined later?
            if !invocations.iter().any(|&c| does_post_dominate(cfga, c, id)) {
                return;
            }
            // Was one already defined, so it's actually a re-definition?
            if invocations.iter().any(|&c| does_post_dominate(cfga, id, c)) {
                return;
            }
            c.err(id, 2218, "This function is only defined later. Move the definition up.");
        },
        &mut |_| {},
    );
    c.into_vec()
}

#[cfg(test)]
mod tests {
    use shelllint_analyzer::testing::run_check;

    fn codes(s: &str) -> Vec<i64> {
        run_check(crate::CHECKS, "Analytics.hs", "checkUseBeforeDefinition", "tree", s).expect("registered")
    }
    fn yes(s: &str) {
        assert_eq!(codes(s), vec![2218], "{s:?}");
    }
    fn no(s: &str) {
        assert!(codes(s).is_empty(), "{s:?}");
    }


    #[test]
    fn t1() { yes("f; f() { true; }"); }
    #[test]
    fn t2() { no("f() { true; }; f"); }
    #[test]
    fn t3() { no("if ! mycmd --version; then mycmd() { true; }; fi"); }
    #[test]
    fn t4() { no("mycmd || mycmd() { f; }"); }
    #[test]
    fn t5() { yes("false || mycmd; mycmd() { f; }"); }
    #[test]
    fn t6() { no("f() { one; }; f; f() { two; }; f"); }
    #[test]
    fn t7() { no("f() { true; }\nreturn\necho \"$(f)\"\necho \"$(f)\""); }
    #[test]
    fn t8() { no("#!/usr/bin/env bats\n@test 'foo' { f; }\nf() { true; }"); }
    #[test]
    fn t9() { no("#!/usr/bin/env bats\n@test 'foo' { f; }\nsetup() { f() { true; }; }"); }
    #[test]
    fn t10() { yes("#!/usr/bin/env bats\n@test 'foo' {\n f\n f() { true; }\n}"); }
    #[test]
    fn t11() { yes("#!/usr/bin/env bats\nf\n@test 'foo' { true; }\nf() { true; }"); }
}
