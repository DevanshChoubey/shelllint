// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkDeprecatedTempfile`, lines 1049-1051 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::get_command_token_or_this;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkDeprecatedTempfile",
    registry: "command",
    order: 26,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("tempfile"), check_deprecated_tempfile),
};

fn check_deprecated_tempfile(p: &Params<'_>, t: Id, c: &mut Comments) {
    c.warn(get_command_token_or_this(p.ast, t), 2186, "tempfile is deprecated. Use mktemp instead.");
}
