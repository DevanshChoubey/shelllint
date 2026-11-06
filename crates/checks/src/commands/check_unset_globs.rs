// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkUnsetGlobs`, lines 950-956 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, is_glob};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkUnsetGlobs",
    registry: "command",
    order: 21,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("unset"), check_unset_globs),
};

/// `mapM_ check . arguments`
fn check_unset_globs(p: &Params<'_>, t: Id, c: &mut Comments) {
    for arg in arguments(p.ast, t) {
        check(p, arg, c);
    }
}

fn check(p: &Params<'_>, arg: Id, c: &mut Comments) {
    if is_glob(p.ast, arg) {
        c.warn(arg, 2184, "Quote arguments to unset so they're not glob expanded.");
    }
}
