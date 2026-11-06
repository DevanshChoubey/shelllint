// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkBangAfterPipe`, lines 703-714 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkBangAfterPipe",
    registry: "shell-support",
    order: 7,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Dash, Shell::BusyboxSh, Shell::Sh, Shell::Bash], f),
};

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Pipeline(_, cmds) = p.ast.get(t) {
        for &cmd in cmds {
            check(p, cmd, c);
        }
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Banged(_) = p.ast.get(t) {
        c.err(
            t,
            2326,
            "! is not allowed in the middle of pipelines. Use command group as in cmd | { ! cmd; } if necessary.",
        );
    }
}
