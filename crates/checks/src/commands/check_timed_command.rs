// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkTimedCommand`, lines 1000-1022 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::{Id, Token};
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkTimedCommand",
    registry: "command",
    order: 24,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("time"), check_timed_command),
};

fn check_timed_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    // (c:args@(_:_))
    if words.len() < 2 {
        return;
    }
    let cid = words[0];
    let args = &words[1..];
    // `whenShell [Sh, Dash, BusyboxSh]`, tested directly.
    if ![Shell::Sh, Shell::Dash, Shell::BusyboxSh].contains(&p.shell_type) {
        return;
    }
    let cmd = *args.last().expect("non-empty");
    if is_piped(p, cmd) {
        c.warn(cid, 2176, "'time' is undefined for pipelines. time single stage or bash -c instead.");
    }
    if is_simple(p, cmd) == Some(false) {
        c.warn(cmd, 2177, "'time' is undefined for compound commands, time sh -c instead.");
    }
}

fn is_piped(p: &Params<'_>, cmd: Id) -> bool {
    matches!(p.ast.get(cmd), Token::T_Pipeline(_, cmds) if cmds.len() >= 2)
}

fn get_command(p: &Params<'_>, cmd: Id) -> Option<Id> {
    match p.ast.get(cmd) {
        Token::T_Pipeline(_, cmds) => match p.ast.get(*cmds.first()?) {
            Token::T_Redirecting(_, a) => Some(*a),
            _ => None,
        },
        _ => None,
    }
}

fn is_simple(p: &Params<'_>, cmd: Id) -> Option<bool> {
    let inner = get_command(p, cmd)?;
    Some(matches!(p.ast.get(inner), Token::T_SimpleCommand(..)))
}
