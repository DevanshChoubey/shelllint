// SPDX-License-Identifier: GPL-3.0-or-later
// Performance regression tests for the analyzer (CFG build, the cfIdToNodes
// lookup, `builtin X` dispatch); no Haskell counterpart.
//
// Each test re-runs itself in a child process (this test binary, `--exact
// <name>`, with `SC_ANALYZER_PATHOLOGICAL_CHILD` set) whose address space is
// capped with `ulimit -v`, so a regression fails the test instead of
// exhausting the machine's memory. The parent kills the child after the
// time limit. Every input used to take well over the limit because some
// per-item work scanned or copied the whole script.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use shellcheck_analyzer::analyzerlib::{Comments, Params, default_spec, make_parameters, p_script, run_checker};
use shellcheck_analyzer::api::{CommandName, NodeFn, get_checker};
use shellcheck_analyzer::cfg::{CfgParameters, build_graph};
use shellcheck_analyzer::cfg_analysis::analyze_control_flow;
use shellcheck_core::ast::{Id, Token};

const CHILD_ENV: &str = "SC_ANALYZER_PATHOLOGICAL_CHILD";
const TIME_LIMIT: Duration = Duration::from_secs(10);
/// Address-space cap for the child, in KB.
const AS_LIMIT_KB: u64 = 2 * 1024 * 1024;

fn check(name: &str, work: impl FnOnce()) {
    if std::env::var_os(CHILD_ENV).is_some() {
        work();
        println!("CHILD_DONE");
        return;
    }
    let start = Instant::now();
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("ulimit -v {AS_LIMIT_KB} && exec \"$0\" \"$@\""))
        .arg(&exe)
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_reader = std::thread::spawn(move || {
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        err
    });
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        let err = err_reader.join().unwrap_or_default();
        let _ = tx.send((out, err));
    });
    let (out, err) = match rx.recv_timeout(TIME_LIMIT) {
        Ok(o) => o,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{name}: took longer than {TIME_LIMIT:?}");
        }
    };
    let status = child.wait().unwrap();
    let elapsed = start.elapsed();
    assert!(status.success(), "{name}: child failed ({status}):\n{out}\n{err}");
    assert!(out.contains("CHILD_DONE"), "{name}: child did not run:\n{out}\n{err}");
    eprintln!("{name}: {elapsed:?}");
    assert!(elapsed < TIME_LIMIT, "{name}: took {elapsed:?}");
}

fn parse(script: &str) -> (shellcheck_parser::ParseResult, Id) {
    let mut pr = p_script(script);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    (pr, root)
}

/// One simple command with 20000 words: the structural-node chain collapse
/// in build_graph followed every chain from scratch (27 s before the fix).
#[test]
fn cfg_one_command_with_many_words() {
    check("cfg_one_command_with_many_words", || {
        let words: Vec<String> = (0..20_000).map(|i| format!("x{i}")).collect();
        let script = format!("#!/bin/sh\ncat <x {}\n", words.join(" "));
        let (pr, root) = parse(&script);
        let g = build_graph(&CfgParameters::default(), &pr.ast, root);
        assert!(g.cf_id_to_range(root).is_some());
        let _ = analyze_control_flow(&CfgParameters::default(), &pr.ast, root);
    });
}

/// `token_to_nodes` for every token of 8000 declaring commands: each query
/// used to scan every CFG node and its token stack.
#[test]
fn cfg_id_to_nodes_for_every_token() {
    check("cfg_id_to_nodes_for_every_token", || {
        let mut script = String::from("#!/bin/bash\n");
        for i in 0..8000 {
            script.push_str(&format!("declare a{i}=1 b{i}=\"$c\"\n"));
        }
        let (pr, root) = parse(&script);
        let a = analyze_control_flow(&CfgParameters::default(), &pr.ast, root);
        let mut found = 0usize;
        for id in 0..pr.ast.len() {
            if let Some(ns) = a.token_to_nodes(Id(id)) {
                found += ns.len();
            }
        }
        assert!(found > 0);
        // The root owns every node.
        assert!(a.token_to_nodes(root).is_some_and(|ns| ns.len() > 8000));
    });
}

fn count_args(p: &Params<'_>, t: Id, c: &mut Comments) {
    // The rewritten command must be `echo ...`, not `builtin echo ...`.
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { panic!("not a command") };
    assert_eq!(words.len(), 7);
    let _ = c;
}

/// 4000 `builtin echo` commands: each one used to copy the whole arena and
/// the variable flow (27 s through the CLI before the fix).
#[test]
fn builtin_dispatch_does_not_copy_the_script() {
    check("builtin_dispatch_does_not_copy_the_script", || {
        let mut script = String::from("#!/bin/bash\n");
        for i in 0..4000 {
            script.push_str(&format!("builtin echo \"a{i}\" b c d e f\n"));
        }
        let (pr, root) = parse(&script);
        let params = make_parameters(&default_spec(&pr), &pr.ast, root);
        let list: Vec<(CommandName, NodeFn)> = vec![(CommandName::Exactly("echo"), count_args)];
        let checker = get_checker(&list);
        for _ in 0..3 {
            let _ = run_checker(&params, &checker);
        }
    });
}

/// Word counts of every simple command, as the check sees the arena.
fn record_word_counts(p: &Params<'_>, t: Id, c: &mut Comments) {
    let mut counts: Vec<String> = vec![];
    p.ast.analyze(
        p.root,
        &mut |i| {
            if let Token::T_SimpleCommand(_, w) = p.ast.get(i) {
                counts.push(format!("{}{}", if i == t { "*" } else { "" }, w.len()));
            }
        },
        &mut |_| {},
    );
    c.0.push(shellcheck_core::interface::TokenComment {
        id: t,
        comment: shellcheck_core::interface::Comment {
            severity: shellcheck_core::interface::Severity::InfoC,
            code: 0,
            message: counts.join(" "),
        },
        fix: None,
    });
}

/// Only the command being checked is rewritten; the reused copy is put
/// back between commands, so every other command keeps its `builtin`.
#[test]
fn builtin_dispatch_rewrites_only_the_checked_command() {
    let (pr, root) = parse("#!/bin/bash\nbuiltin echo a\nbuiltin echo b c\necho d\n");
    let params = make_parameters(&default_spec(&pr), &pr.ast, root);
    let list: Vec<(CommandName, NodeFn)> = vec![(CommandName::Exactly("echo"), record_word_counts)];
    let checker = get_checker(&list);
    let got: Vec<String> = run_checker(&params, &checker).into_iter().map(|c| c.comment.message).collect();
    assert_eq!(got, vec!["*2 4 2", "3 *3 2", "3 4 *2"]);
}
