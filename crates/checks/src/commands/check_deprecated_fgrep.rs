// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkDeprecatedFgrep`, lines 1057-1059 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::get_command_token_or_this;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkDeprecatedFgrep",
    registry: "command",
    order: 28,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("fgrep"), check_deprecated_fgrep),
};

fn check_deprecated_fgrep(p: &Params<'_>, t: Id, c: &mut Comments) {
    c.info(
        get_command_token_or_this(p.ast, t),
        2197,
        "fgrep is non-standard and deprecated. Use grep -F instead.",
    );
}
