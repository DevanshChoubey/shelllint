// SPDX-License-Identifier: GPL-3.0-only
// Port of the prop_ tests in ShellCheck src/ShellCheck/AnalyzerLib.hs (e11f964),
// plus tests of the check registry (src/api.rs) on hand-built trees.
//
// The 13 determineShell tests parse a script, so they are
// `#[ignore = "needs parser"]` until the parser is ported.
use std::panic::{AssertUnwindSafe, catch_unwind};

use shellcheck_analyzer::analyzerlib::*;
use shellcheck_analyzer::api::*;
use shellcheck_analyzer::testing::{find_entries, run_check};
use shellcheck_core::ast::{AssignmentMode, Ast, Id, Pos, Token};
use shellcheck_core::interface::*;

// ---- prop_ tests ----

#[test]
#[ignore = "needs parser"]
fn determine_shell0() {
    assert_eq!(determine_shell_test("#!/bin/sh"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell1() {
    assert_eq!(determine_shell_test("#!/usr/bin/env ksh"), Shell::Ksh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell2() {
    assert_eq!(determine_shell_test(""), Shell::Bash);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell3() {
    assert_eq!(determine_shell_test("#!/bin/sh -e"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell4() {
    assert_eq!(determine_shell_test("#!/bin/ksh\n#shellcheck shell=sh\nfoo"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell5() {
    assert_eq!(determine_shell_test("#shellcheck shell=sh\nfoo"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell6() {
    assert_eq!(determine_shell_test("#! /bin/sh"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell7() {
    assert_eq!(determine_shell_test("#! /bin/ash"), Shell::Dash);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell8() {
    assert_eq!(determine_shell_test_(Some(Shell::Ksh), "#!/bin/sh"), Shell::Sh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell9() {
    assert_eq!(determine_shell_test("#!/bin/env -S dash -x"), Shell::Dash);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell10() {
    assert_eq!(determine_shell_test("#!/bin/env --split-string= dash -x"), Shell::Dash);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell11() {
    // busybox sh is a specific shell, not posix sh
    assert_eq!(determine_shell_test("#!/bin/busybox sh"), Shell::BusyboxSh);
}
#[test]
#[ignore = "needs parser"]
fn determine_shell12() {
    assert_eq!(determine_shell_test("#!/bin/busybox ash"), Shell::BusyboxSh);
}

#[test]
fn get_variables_from_literal1() {
    assert_eq!(get_variables_from_literal("$foo${bar//a/b}$BAZ"), vec!["foo", "bar", "BAZ"]);
}

// ---- hand-built trees ----

/// Tiny arena builder: ids are allocated in call order.
struct B(Ast);

impl B {
    fn new() -> Self {
        B(Ast::new())
    }
    fn t(&mut self, tok: Token) -> Id {
        let p = Pos { line: 1, column: 1 };
        let id = self.0.alloc(p, p);
        self.0.set(id, tok);
        id
    }
    fn lit(&mut self, s: &str) -> Id {
        self.t(Token::T_Literal(s.to_string()))
    }
    fn word(&mut self, s: &str) -> Id {
        let l = self.lit(s);
        self.t(Token::T_NormalWord(vec![l]))
    }
    /// `T_Script` with shebang `sb` around one simple command per argv.
    fn script(mut self, sb: &str, cmds: &[&[&str]]) -> (Ast, Id) {
        let sb = self.lit(sb);
        let mut list = vec![];
        for argv in cmds {
            let words: Vec<Id> = argv.iter().map(|w| self.word(w)).collect();
            let sc = self.t(Token::T_SimpleCommand(vec![], words));
            list.push(self.t(Token::T_Redirecting(vec![], sc)));
        }
        let root = self.t(Token::T_Script(sb, list));
        let mut ast = self.0;
        ast.compute_parents(root);
        (ast, root)
    }
}

fn spec() -> AnalysisSpec {
    new_analysis_spec()
}

fn codes(v: &[TokenComment]) -> Vec<Code> {
    v.iter().map(|c| c.comment.code).collect()
}

#[test]
fn empty_fix_is_none() {
    let c = make_comment_with_fix(Severity::WarningC, Id(3), 2000, "msg", new_fix());
    assert_eq!(c.fix, None);
    let fix = Fix { replacements: vec![new_replacement()] };
    let c = make_comment_with_fix(Severity::WarningC, Id(3), 2000, "msg", fix.clone());
    assert_eq!(c.fix, Some(fix));

    let mut cs = Comments::new();
    cs.warn_with_fix(Id(1), 1, "a", new_fix());
    cs.err(Id(2), 2, "b");
    cs.info(Id(3), 3, "c".to_string());
    cs.style(Id(4), 4, "d");
    let v = cs.into_vec();
    assert_eq!(v[0].fix, None);
    assert_eq!(v[0].comment.severity, Severity::WarningC);
    assert_eq!(
        v.iter().map(|c| c.comment.severity).collect::<Vec<_>>(),
        [Severity::WarningC, Severity::ErrorC, Severity::InfoC, Severity::StyleC]
    );
    assert_eq!(codes(&v), [1, 2, 3, 4]);
}

fn noop(_: &Params<'_>, _: Id, _: &mut Comments) {}

// Registered from this (separate) crate: exercises linkme across crates.
#[linkme::distributed_slice(CHECKS)]
static SORT_FIVE: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "checkFive",
    registry: "test-sort",
    order: 5,
    optional: None,
    kind: CheckKind::Node(noop),
};

#[linkme::distributed_slice(CHECKS)]
static SORT_TWO: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "checkTwo",
    registry: "test-sort",
    order: 2,
    optional: None,
    kind: CheckKind::Node(noop),
};

#[test]
fn registry_sorts_by_order() {
    let orders: Vec<u32> = registered("test-sort").iter().map(|e| e.order).collect();
    assert_eq!(orders, [2, 5]);
    assert!(registered("no-such-registry").is_empty());
}

fn warn_on_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    // Code = number of words the check sees, so the builtin rewrite is visible.
    if let Token::T_SimpleCommand(_, w) = p.ast.get(t) {
        c.warn(t, 1000 + w.len() as Code, "cmd");
    }
}

fn warn_on_command_b(_: &Params<'_>, t: Id, c: &mut Comments) {
    c.warn(t, 2, "b");
}

#[test]
fn command_dispatch() {
    let checker = || {
        get_checker(&[
            (CommandName::Exactly("tr"), warn_on_command as NodeFn),
            (CommandName::Basename("sudo"), warn_on_command as NodeFn),
        ])
    };
    let run = |cmds: &[&[&str]]| {
        let (ast, root) = B::new().script("", cmds);
        let p = make_parameters(&spec(), &ast, root);
        codes(&run_checker(&p, &checker()))
    };
    assert_eq!(run(&[&["tr", "a", "b"]]), [1003]);
    assert_eq!(run(&[&["/usr/bin/tr", "a"]]), Vec::<Code>::new()); // Exactly: no path match
    assert_eq!(run(&[&["/usr/bin/sudo", "ls"]]), [1002]);
    assert_eq!(run(&[&["sudo", "ls"]]), [1002]);
    // builtin tr a b: the check sees `tr a b` (3 words) with the same id
    assert_eq!(run(&[&["builtin", "tr", "a", "b"]]), [1003]);
    assert_eq!(run(&[&["builtin", "sudo"]]), Vec::<Code>::new()); // builtin looks up Exactly only
    assert_eq!(run(&[&["ls"]]), Vec::<Code>::new());
}

#[test]
fn command_map_runs_later_insertions_first() {
    // M.insertWith composeAnalyzers: the new function runs before the old.
    let checker = get_checker(&[
        (CommandName::Exactly("tr"), warn_on_command as NodeFn),
        (CommandName::Exactly("tr"), warn_on_command_b as NodeFn),
    ]);
    let (ast, root) = B::new().script("", &[&["tr"]]);
    let p = make_parameters(&spec(), &ast, root);
    assert_eq!(codes(&run_checker(&p, &checker)), [2, 1001]);
}

fn script_check(_: &Params<'_>, t: Id, c: &mut Comments) {
    c.info(t, 1, "script");
}
fn token_check_a(p: &Params<'_>, t: Id, c: &mut Comments) {
    if matches!(p.ast.get(t), Token::T_SimpleCommand(..)) {
        c.info(t, 10, "a");
    }
}
fn token_check_b(p: &Params<'_>, t: Id, c: &mut Comments) {
    if matches!(p.ast.get(t), Token::T_SimpleCommand(..)) {
        c.info(t, 20, "b");
    }
}

#[test]
fn run_checker_composition_order() {
    let x = Checker { per_script: vec![Box::new(script_check)], per_token: vec![Box::new(token_check_a)] };
    let y = Checker { per_script: vec![Box::new(script_check)], per_token: vec![Box::new(token_check_b)] };
    let (ast, root) = B::new().script("", &[&["a"], &["b"]]);
    let p = make_parameters(&spec(), &ast, root);
    // perScript of both, then one pre-order walk calling a then b per node.
    let out = run_checker(&p, &Checker::mconcat([x, y]));
    assert_eq!(codes(&out), [1, 1, 10, 20, 10, 20]);
    assert!(out[2].id < out[4].id);
}

fn ksh_only(_: &Params<'_>, t: Id, c: &mut Comments) {
    c.warn(t, 3000, "ksh");
}

#[linkme::distributed_slice(CHECKS)]
static SHELL_ENTRY: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "kshOnly",
    registry: "test-shell",
    order: 0,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Ksh], ksh_only),
};

#[test]
fn make_parameters_from_shebang_and_spec() {
    let (ast, root) = B::new().script("/bin/ksh", &[&["set", "-e"], &["set", "-o", "pipefail"]]);
    let p = make_parameters(&spec(), &ast, root);
    assert_eq!(p.shell_type, Shell::Ksh);
    assert!(!p.shell_type_specified);
    assert!(p.has_errexit && p.has_pipefail && p.has_lastpipe);
    assert!(!p.has_allexport && !p.has_noglob && !p.has_inherit_errexit && !p.has_execfail);
    assert!(p.cfg_analysis.is_some());

    let s = AnalysisSpec { shell_type: Some(Shell::Dash), extended_analysis: Some(false), ..spec() };
    let p = make_parameters(&s, &ast, root);
    assert_eq!(p.shell_type, Shell::Dash);
    assert!(p.shell_type_specified && p.has_inherit_errexit && !p.has_lastpipe);
    assert!(p.cfg_analysis.is_none());

    let s = AnalysisSpec { fallback_shell: Some(Shell::Sh), ..spec() };
    let (ast2, root2) = B::new().script("", &[&["shopt", "-s", "lastpipe"]]);
    assert_eq!(make_parameters(&s, &ast2, root2).shell_type, Shell::Sh);
    let p = make_parameters(&spec(), &ast2, root2);
    assert_eq!(p.shell_type, Shell::Bash);
    assert!(p.has_lastpipe);
}

#[test]
fn shell_support_filters_by_shell() {
    let (ast, root) = B::new().script("/bin/ksh", &[&["x"]]);
    let p = make_parameters(&spec(), &ast, root);
    let entries = registered("test-shell");
    // The module checker reads "shell-support" only; build the same from our entry.
    let ksh = shell_support_checker(&p);
    assert!(ksh.per_token.len() == registered("shell-support").len());
    let test = entry_test_checker(&entries);
    assert!(!run_checker(&p, &test).is_empty());
    let s = AnalysisSpec { shell_type: Some(Shell::Bash), ..spec() };
    let p = make_parameters(&s, &ast, root);
    // testChecker ignores the shell list
    assert!(!run_checker(&p, &entry_test_checker(&entries)).is_empty());
}

#[test]
fn variable_flow_for_assignment_and_read() {
    // a=b; read -r x
    let mut b = B::new();
    let sb = b.lit("");
    let val = b.word("b");
    let asg = b.t(Token::T_Assignment(AssignmentMode::Assign, "a".into(), vec![], val));
    let sc1 = b.t(Token::T_SimpleCommand(vec![asg], vec![]));
    let r1 = b.t(Token::T_Redirecting(vec![], sc1));
    let w: Vec<Id> = ["read", "-r", "x"].iter().map(|s| b.word(s)).collect();
    let sc2 = b.t(Token::T_SimpleCommand(vec![], w));
    let r2 = b.t(Token::T_Redirecting(vec![], sc2));
    let root = b.t(Token::T_Script(sb, vec![r1, r2]));
    let mut ast = b.0;
    ast.compute_parents(root);
    let p = make_parameters(&spec(), &ast, root);
    let names: Vec<String> = p
        .variable_flow
        .iter()
        .filter_map(|s| match s {
            StackData::Assignment(base, _, n, _) => Some(format!("{}@{}", n, base.0)),
            _ => None,
        })
        .collect();
    assert_eq!(names, [format!("a@{}", asg.0), format!("x@{}", sc2.0)]);
}

#[test]
fn filter_by_annotation_drops_disabled_codes() {
    let mut b = B::new();
    let sb = b.lit("");
    let w = b.word("ls");
    let sc = b.t(Token::T_SimpleCommand(vec![], vec![w]));
    let ann = b.t(Token::T_Annotation(vec![shellcheck_core::ast::Annotation::DisableComment(2000, 2001)], sc));
    let root = b.t(Token::T_Script(sb, vec![ann]));
    let mut ast = b.0;
    ast.compute_parents(root);
    let p = make_parameters(&spec(), &ast, root);
    let notes = vec![
        make_comment(Severity::WarningC, w, 2000, "x"),
        make_comment(Severity::WarningC, w, 2001, "y"),
        make_comment(Severity::WarningC, root, 2000, "z"),
    ];
    assert_eq!(codes(&filter_by_annotation(&spec(), &p, notes)), [2001, 2000]);
    assert!(should_ignore_code(&p, 2000, w));
    assert!(!should_ignore_code(&p, 2000, root));
}

#[linkme::distributed_slice(CHECKS)]
static RUN_CHECK_ENTRY: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "checkRunCheck",
    registry: "test",
    order: 0,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("sudo"), warn_on_command),
};

#[test]
fn run_check_lookup() {
    assert_eq!(run_check("Test.hs", "checkMissing", "node", "true"), None);
    assert_eq!(run_check("Other.hs", "checkRunCheck", "node", "true"), None);
    assert_eq!(find_entries("Test.hs", "checkRunCheck \"sudo\"").len(), 1);
    assert!(find_entries("Test.hs", "checkRunCheck \"doas\"").is_empty());
    // With the parser stubbed, running a registered check is `blocked`.
    let r = catch_unwind(AssertUnwindSafe(|| run_check("Test.hs", "checkRunCheck", "node", "sudo x")));
    match r {
        Ok(codes) => assert_eq!(codes, Some(vec![1002])),
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            assert!(msg.starts_with("unported:"), "{msg}");
        }
    }
}

#[test]
#[should_panic(expected = "wrong CheckKind")]
fn run_check_rejects_kind_mismatch() {
    run_check("Test.hs", "checkRunCheck", "tree", "sudo x");
}

#[test]
#[ignore = "needs parser"]
fn run_check_runs_command_check() {
    assert_eq!(run_check("Test.hs", "checkRunCheck", "node", "sudo x"), Some(vec![1002]));
    assert_eq!(run_check("Test.hs", "checkRunCheck \"sudo\"", "node", "ls"), Some(vec![]));
}
