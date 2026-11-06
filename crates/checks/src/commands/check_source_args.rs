// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSourceArgs`, lines 1322-1329 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, when_shell};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::arguments;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
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
