// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkDeprecatedEgrep`, lines 1053-1055 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::get_command_token_or_this;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkDeprecatedEgrep",
    registry: "command",
    order: 27,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("egrep"), check_deprecated_egrep),
};

fn check_deprecated_egrep(p: &Params<'_>, t: Id, c: &mut Comments) {
    c.info(
        get_command_token_or_this(p.ast, t),
        2196,
        "egrep is non-standard and deprecated. Use grep -E instead.",
    );
}
