// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkChmodDashr`, lines 1333-1340 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkChmodDashr",
    registry: "command",
    order: 38,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("chmod"), check_chmod_dashr),
};

/// `mapM_ check $ arguments t`
fn check_chmod_dashr(p: &Params<'_>, t: Id, c: &mut Comments) {
    for a in arguments(p.ast, t) {
        check(p, a, c);
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Some(flag) = get_literal_string(p.ast, t) else { return };
    if flag != "-r" {
        return;
    }
    c.warn(t, 2253, "Use -R to recurse, or explicitly a-r to remove read permissions.");
}
