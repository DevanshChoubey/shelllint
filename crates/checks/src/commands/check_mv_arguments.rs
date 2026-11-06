// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMvArguments`, lines 1236-1239 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::missing_destination;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMvArguments",
    registry: "command",
    order: 32,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("mv"), check),
};

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    missing_destination(f, p.ast, t, c);
}

fn f(t: Id, c: &mut Comments) {
    c.err(t, 2224, "This mv has no destination. Check the arguments.");
}
