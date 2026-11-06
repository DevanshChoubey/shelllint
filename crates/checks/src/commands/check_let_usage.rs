// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkLetUsage`, lines 1209-1214 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, when_shell};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkLetUsage",
    registry: "command",
    order: 31,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("let"), check_let_usage),
};

fn check_let_usage(p: &Params<'_>, t: Id, c: &mut Comments) {
    when_shell(p, &[Shell::Bash, Shell::Ksh], || {
        c.style(t, 2219, "Instead of 'let expr', prefer (( expr )) .");
    });
}
