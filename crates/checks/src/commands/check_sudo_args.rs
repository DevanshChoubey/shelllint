// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSudoArgs`, lines 1307-1318 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_bsd_opts, get_literal_string};

use crate::support::parametrised_command_checks;

parametrised_command_checks!("checkSudoArgs", 49, Basename, check_sudo_args_, ["sudo", "doas", "run0"]);

/// `checkSudoArgs cmd = CommandCheck (Basename cmd) f`
fn check_sudo_args_(_cmd: &str, p: &Params<'_>, t: Id, c: &mut Comments) {
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
