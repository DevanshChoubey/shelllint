// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkWhich`, lines 1267-1269 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName, StaticCheckDescription};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::get_command_token_or_this;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkWhich",
    registry: "optional-command",
    order: 0,
    optional: Some(StaticCheckDescription {
        name: "deprecate-which",
        id: &[2230],
        description: "Suggest 'command -v' instead of 'which'",
        positive: "which javac",
        negative: "command -v javac",
    }),
    kind: CheckKind::Command(CommandName::Basename("which"), check_which),
};

fn check_which(p: &Params<'_>, t: Id, c: &mut Comments) {
    c.info(
        get_command_token_or_this(p.ast, t),
        2230,
        "'which' is non-standard. Use builtin 'command -v' instead.",
    );
}
