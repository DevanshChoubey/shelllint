// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkReturn`, lines 490-493 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::return_or_exit;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkReturn",
    registry: "command",
    order: 5,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("return"), check_return),
};

fn check_return(p: &Params<'_>, t: Id, c: &mut Comments) {
    return_or_exit(
        |id, c| c.err(id, 2151, "Only one integer 0-255 can be returned. Use stdout for other data."),
        |id, c| c.err(id, 2152, "Can only return 0-255. Other data should be written to stdout."),
        p.ast,
        t,
        c,
    );
}
