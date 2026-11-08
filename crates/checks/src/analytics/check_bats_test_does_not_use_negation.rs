// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkBatsTestDoesNotUseNegation`, lines 5216-5240 (e11f964)

use crate::support::{fix_with, replace_start};
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkBatsTestDoesNotUseNegation",
    registry: "node",
    order: 94,
    optional: None,
    kind: CheckKind::Node(check_bats_test_does_not_use_negation),
};

fn check_bats_test_does_not_use_negation(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_BatsTest(_, body) = p.ast.get(t) else { return };
    let Token::T_BraceGroup(commands) = p.ast.get(*body) else { return };
    for &cmd in commands {
        check(p, c, commands, cmd);
    }
}

fn check(p: &Params<'_>, c: &mut Comments, commands: &[Id], t: Id) {
    let Token::T_Banged(inner) = p.ast.get(t) else { return };
    let id = t;
    let is_cond = match p.ast.get(*inner) {
        Token::T_Pipeline(_, cmds) => match &cmds[..] {
            [r] => match p.ast.get(*r) {
                Token::T_Redirecting(_, cond) => matches!(p.ast.get(*cond), Token::T_Condition(..)),
                _ => false,
            },
            _ => false,
        },
        _ => false,
    };
    let last = is_last_of(p, t, commands);
    if is_cond {
        if last {
            c.style(id, 2315, "In Bats, ! will not fail the test if it is not the last command anymore. Fold the `!` into the conditional!");
        } else {
            c.err(id, 2315, "In Bats, ! does not cause a test failure. Fold the `!` into the conditional!");
        }
    } else if last {
        c.style_with_fix(id, 2314, "In Bats, ! will not fail the test if it is not the last command anymore. Use `run ! ` (on Bats >= 1.5.0) instead.",
            fix_with(vec![replace_start(id, p, 0, "run ")]));
    } else {
        c.err_with_fix(id, 2314, "In Bats, ! does not cause a test failure. Use 'run ! ' (on Bats >= 1.5.0) instead.",
            fix_with(vec![replace_start(id, p, 0, "run ")]));
    }
}

fn is_last_of(p: &Params<'_>, t: Id, commands: &[Id]) -> bool {
    match commands {
        [x] => p.ast.structurally_eq(*x, t),
        [_, rest @ ..] => is_last_of(p, t, rest),
        [] => false,
    }
}
