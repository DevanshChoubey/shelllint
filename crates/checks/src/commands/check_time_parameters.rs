// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkTimeParameters`, lines 987-996 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, when_shell};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;
use shellcheck_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkTimeParameters",
    registry: "command",
    order: 23,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("time"), check_time_parameters),
};

fn check_time_parameters(p: &Params<'_>, t: Id, c: &mut Comments) {
    f(p, t, c)
}

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else {
        return;
    };
    let [cmd, args, ..] = words.as_slice() else {
        return;
    };
    let (cmd, args) = (*cmd, *args);
    when_shell(p, &[Shell::Bash, Shell::Sh], || {
        let s = oversimplify(p.ast, args).concat();
        if s.starts_with('-') && s != "-p" {
            c.info(
                cmd,
                2023,
                "The shell may override 'time' as seen in man time(1). Use 'command time ..' for that one.",
            );
        }
    });
}
