// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkCpArguments`, lines 1240-1243 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::missing_destination;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkCpArguments",
    registry: "command",
    order: 33,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("cp"), check),
};

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    missing_destination(f, p.ast, t, c);
}

fn f(t: Id, c: &mut Comments) {
    c.err(t, 2225, "This cp has no destination. Check the arguments.");
}
