// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkMultipleBangs`, lines 692-699 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkMultipleBangs",
    registry: "shell-support",
    order: 6,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Dash, Shell::BusyboxSh, Shell::Sh], f),
};

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Banged(inner) = p.ast.get(t) {
        if let Token::T_Banged(_) = p.ast.get(*inner) {
            c.err(
                t,
                2325,
                "Multiple ! in front of pipelines are a bash/ksh extension. Use only 0 or 1.",
            );
        }
    }
}
