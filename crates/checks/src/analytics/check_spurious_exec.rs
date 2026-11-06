// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSpuriousExec`, lines 2038-2074 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command_match};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::{get_literal_string, is_assignment};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSpuriousExec",
    registry: "node",
    order: 38,
    optional: None,
    kind: CheckKind::Node(check_spurious_exec),
};

fn check_spurious_exec(p: &Params<'_>, t: Id, c: &mut Comments) {
    if p.has_execfail {
        return;
    }
    do_lists(p.ast, t, c);
}

fn do_lists(ast: &Ast, t: Id, c: &mut Comments) {
    match ast.get(t) {
        Token::T_Script(_, cmds) => do_list(ast, cmds, false, c),
        Token::T_BraceGroup(cmds) => do_list(ast, cmds, false, c),
        Token::T_WhileExpression(_, cmds) => do_list(ast, cmds, true, c),
        Token::T_UntilExpression(_, cmds) => do_list(ast, cmds, true, c),
        Token::T_ForIn(_, _, cmds) => do_list(ast, cmds, true, c),
        Token::T_ForArithmetic(_, _, _, cmds) => do_list(ast, cmds, true, c),
        Token::T_IfExpression(thens, elses) => {
            for (_, l) in thens {
                do_list(ast, l, false, c);
            }
            do_list(ast, elses, false, c);
        }
        _ => {}
    }
}

fn cleanup(ast: &Ast, t: Id) -> bool {
    if let Token::T_Pipeline(_, cmds) = ast.get(t) {
        if let &[cmd] = &cmds[..] {
            return is_command_match(ast, cmd, &|s| {
                matches!(s, ":" | "echo" | "exit" | "printf" | "return")
            }) || is_assignment(ast, cmd);
        }
    }
    false
}

fn strip_cleanup<'a>(ast: &Ast, l: &'a [Id]) -> &'a [Id] {
    let mut n = l.len();
    while n > 0 && cleanup(ast, l[n - 1]) {
        n -= 1;
    }
    &l[..n]
}

// The loop flag is true if we are in a loop. In that case we should emit the
// warning also if `exec' is the last statement.
fn do_list(ast: &Ast, l: &[Id], in_loop: bool, c: &mut Comments) {
    let l = strip_cleanup(ast, l);
    let upto = if in_loop { l.len() } else { l.len().saturating_sub(1) };
    for &current in &l[..upto] {
        comment_if_exec(ast, current, c);
    }
}

fn comment_if_exec(ast: &Ast, t: Id, c: &mut Comments) {
    match ast.get(t) {
        Token::T_Pipeline(_, cmds) => {
            if let &[x] = &cmds[..] {
                comment_if_exec(ast, x, c);
            }
        }
        Token::T_Redirecting(_, cmd) => {
            if let Token::T_SimpleCommand(_, words) = ast.get(*cmd) {
                if words.len() >= 2 && get_literal_string(ast, words[0]).as_deref() == Some("exec") {
                    c.warn(
                        *cmd,
                        2093,
                        "Remove \"exec \" if script should continue after this command.",
                    );
                }
            }
        }
        _ => {}
    }
}
