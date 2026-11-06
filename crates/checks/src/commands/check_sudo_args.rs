// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSudoArgs`, lines 1307-1318 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_bsd_opts, get_literal_string};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_SUDO: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSudoArgs \"sudo\"",
    registry: "command",
    order: 49,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("sudo"), sudo),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_DOAS: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSudoArgs \"doas\"",
    registry: "command",
    order: 49,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("doas"), doas),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_RUN0: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSudoArgs \"run0\"",
    registry: "command",
    order: 49,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("run0"), run0),
};

fn sudo(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_sudo_args(p, t, c)
}
fn doas(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_sudo_args(p, t, c)
}
fn run0(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_sudo_args(p, t, c)
}

const BUILTINS: &[&str] = &[
    "cd", "command", "declare", "eval", "exec", "exit", "export", "hash", "history", "local",
    "popd", "pushd", "read", "readonly", "return", "set", "source", "trap", "type", "typeset",
    "ulimit", "umask", "unset", "wait",
];

fn check_sudo_args(p: &Params<'_>, t: Id, c: &mut Comments) {
    // This mess is why ShellCheck prefers not to know.
    let Some(opts) = get_bsd_opts(p.ast, "vAknSbEHPa:g:h:p:u:c:T:r:", &arguments(p.ast, t)) else {
        return;
    };
    let Some((_, (command_arg, _))) = opts.iter().find(|(k, _)| k.is_empty()) else {
        return;
    };
    let Some(command) = get_literal_string(p.ast, *command_arg) else {
        return;
    };
    if !BUILTINS.contains(&command.as_str()) {
        return;
    }
    c.warn(
        t,
        2232,
        &format!(
            "Can't use sudo/doas/run0 with builtins like {}. Did you want sudo/doas/run0 sh -c .. instead?",
            command
        ),
    );
}
