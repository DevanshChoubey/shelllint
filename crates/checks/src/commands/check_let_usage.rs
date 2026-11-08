// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkLetUsage`, lines 1209-1214 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, when_shell};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::Id;
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
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
