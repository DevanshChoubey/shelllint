// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkLocalScope`, lines 1025-1030 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_path_m, when_shell};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{get_command_token_or_this, is_function_like};
use shellcheck_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkLocalScope",
    registry: "command",
    order: 25,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("local"), check_local_scope),
};

fn check_local_scope(p: &Params<'_>, t: Id, c: &mut Comments) {
    // Ksh allows it, Sh doesn't support local
    when_shell(p, &[Shell::Bash, Shell::Dash, Shell::BusyboxSh], || {
        let path = get_path_m(p, t);
        if !path.iter().any(|&x| is_function_like(p.ast, x)) {
            c.err(get_command_token_or_this(p.ast, t), 2168, "'local' is only valid in functions.");
        }
    });
}
