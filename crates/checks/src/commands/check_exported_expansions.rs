// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkExportedExpansions`, lines 865-871 (e11f964)

use crate::support::get_single_unmodified_braced_string;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::arguments;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkExportedExpansions",
    registry: "command",
    order: 18,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("export"), check_exported_expansions),
};

/// `mapM_ check . arguments`
fn check_exported_expansions(p: &Params<'_>, t: Id, c: &mut Comments) {
    for w in arguments(p.ast, t) {
        check(p, w, c);
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Some(name) = get_single_unmodified_braced_string(p.ast, t) {
        c.warn(
            t,
            2163,
            &format!("This does not export '{}'. Remove $/${{}} for that, or use ${{var?}} to quiet.", name),
        );
    }
}
