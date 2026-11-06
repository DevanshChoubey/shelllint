// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFindExecWithSingleArgument`, lines 528-542 (e11f964)

use std::sync::LazyLock;

use regex::Regex;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string, get_literal_string_def};
use shellcheck_core::regex::{matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFindExecWithSingleArgument",
    registry: "command",
    order: 7,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_find_exec_with_single_argument),
};

static COMMAND_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("[ |;]"));

/// `f . arguments` where `f` runs `check` on every tail of the arguments.
fn check_find_exec_with_single_argument(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    for i in 0..args.len() {
        if let [exec, arg, term, ..] = args[i..] {
            check(p, exec, arg, term, c);
        }
    }
}

fn check(p: &Params<'_>, exec: Id, arg: Id, term: Id, c: &mut Comments) {
    let Some(exec_s) = get_literal_string(p.ast, exec) else { return };
    let Some(term_s) = get_literal_string(p.ast, term) else { return };
    let cmd_s = get_literal_string_def(p.ast, " ", arg);

    if !(["-exec", "-execdir", "-ok", "-okdir"].contains(&exec_s.as_str())
        && [";", "+"].contains(&term_s.as_str()))
    {
        return;
    }
    if !matches(&cmd_s, &COMMAND_REGEX) {
        return;
    }
    c.warn(
        exec,
        2150,
        format!("{exec_s} does not invoke a shell. Rewrite or use {exec_s} sh -c .. ."),
    );
}
