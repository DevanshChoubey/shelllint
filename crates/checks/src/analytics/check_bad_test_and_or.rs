// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkBadTestAndOr`, lines 4513-4545 (e11f964)

use crate::support::{fix_with, replace_end};
use shelllint_analyzer::analyzerlib::{Comments, Params, is_test_command};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkBadTestAndOr",
    registry: "node",
    order: 87,
    optional: None,
    kind: CheckKind::Node(check_bad_test_and_or),
};

fn check_bad_test_and_or(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Pipeline(seps, cmds) if cmds.len() >= 2 => check_ors(p, seps, cmds, c),
        &Token::T_Backgrounded(cmd) => check_ands(p, t, cmd, c),
        _ => {}
    }
}

fn check_ors(p: &Params<'_>, seps: &[Id], cmds: &[Id], c: &mut Comments) {
    let mut maybe_seps: Vec<Option<Id>> = seps.iter().map(|&s| Some(s)).collect();
    let before: Vec<Option<Id>> = std::iter::once(None).chain(maybe_seps.iter().copied()).collect();
    maybe_seps.push(None);
    // zip3 truncates to the shortest list
    for ((b, &cmd), a) in before.into_iter().zip(cmds.iter()).zip(maybe_seps) {
        check_test(p, b, cmd, a, c);
    }
}

fn check_test(p: &Params<'_>, before: Option<Id>, cmd: Id, after: Option<Id>, c: &mut Comments) {
    if is_test_command(p.ast, cmd) {
        check_pipe(p, before, c);
        check_pipe(p, after, c);
    }
}

fn check_pipe(p: &Params<'_>, t: Option<Id>, c: &mut Comments) {
    if let Some(id) = t {
        if matches!(p.ast.get(id), Token::T_Pipe(s) if s == "|") {
            c.warn_with_fix(
                id,
                2266,
                "Use || for logical OR. Single | will pipe.",
                fix_with(vec![replace_end(id, p, 0, "|")]),
            );
        }
    }
}

fn check_ands(p: &Params<'_>, id: Id, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        &Token::T_AndIf(_, rhs) => check_ands(p, id, rhs, c),
        &Token::T_OrIf(_, rhs) => check_ands(p, id, rhs, c),
        Token::T_Pipeline(_, list) if !list.is_empty() => {
            let last = *list.last().unwrap();
            check_ands(p, id, last, c)
        }
        _ => {
            if is_test_command(p.ast, t) {
                c.err_with_fix(
                    id,
                    2265,
                    "Use && for logical AND. Single & will background and return true.",
                    fix_with(vec![replace_end(id, p, 0, "&")]),
                );
            }
        }
    }
}
