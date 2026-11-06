// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFindRedirections`, lines 1252-1265 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command_m};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFindRedirections",
    registry: "command",
    order: 35,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_find_redirections),
};

fn check_find_redirections(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Some(redirecting) = get_closest_command_m(p, t) else { return };
    if let Token::T_Redirecting(redirs, cmd) = p.ast.get(redirecting) {
        if let Token::T_SimpleCommand(_, args) = p.ast.get(*cmd) {
            if !redirs.is_empty() && args.len() >= 2 {
                // This assumes IDs are sequential, which is mostly but not always true.
                let min_redir = *redirs.iter().min().unwrap();
                let max_arg = *args.iter().max().unwrap();
                if min_redir < max_arg {
                    c.warn(
                        min_redir,
                        2227,
                        "Redirection applies to the find command itself. Rewrite to work per action (or move to end).",
                    );
                }
            }
        }
    }
}
