// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSourceArgs`, lines 1322-1329 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, when_shell};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::Id;
use shelllint_core::astlib::arguments;
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSourceArgs",
    registry: "command",
    order: 37,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("."), check_source_args),
};

fn check_source_args(p: &Params<'_>, t: Id, c: &mut Comments) {
    when_shell(p, &[Shell::Sh, Shell::Dash], || {
        let args = arguments(p.ast, t);
        if let [_file, arg1, ..] = args.as_slice() {
            c.warn(
                *arg1,
                2240,
                "The dot command does not support arguments in sh/dash. Set them as variables.",
            );
        }
    });
}
