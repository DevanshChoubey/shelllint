// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkExit`, lines 501-504 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::return_or_exit;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkExit",
    registry: "command",
    order: 6,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("exit"), check_exit),
};

fn check_exit(p: &Params<'_>, t: Id, c: &mut Comments) {
    return_or_exit(
        |id, c| c.err(id, 2241, "The exit status can only be one integer 0-255. Use stdout for other data."),
        |id, c| c.err(id, 2242, "Can only exit with status 0-255. Other data should be written to stdout/stderr."),
        p.ast,
        t,
        c,
    );
}
