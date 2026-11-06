// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkUuoeCmd`, lines 835-840 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, token_is_just_command_output};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::arguments;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkUuoeCmd",
    registry: "command",
    order: 16,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("echo"), check_uuoe_cmd),
};

fn check_uuoe_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    // f [token] = ...; f _ = return ()
    if let [token] = args.as_slice() {
        if token_is_just_command_output(p.ast, *token) {
            c.style(*token, 2005, "Useless echo? Instead of 'echo $(cmd)', just use 'cmd'.");
        }
    }
}
