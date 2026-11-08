// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkInteractiveSu`, lines 689-703 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, get_path_m};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::arguments;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkInteractiveSu",
    registry: "command",
    order: 13,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("su"), check_interactive_su),
};

fn check_interactive_su(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    if arguments(p.ast, cmd).len() <= 1 {
        let path = get_path_m(p, cmd);
        if path.iter().all(|&t| undirected(p, t)) {
            c.info(cmd, 2117, "To run commands as another user, use su -c or sudo.");
        }
    }
}

fn undirected(p: &Params<'_>, t: Id) -> bool {
    match p.ast.get(t) {
        Token::T_Pipeline(_, cmds) if cmds.len() >= 2 => false,
        // This should really just be modifications to stdin, but meh
        Token::T_Redirecting(redirs, _) if !redirs.is_empty() => false,
        _ => true,
    }
}
