// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkUnusedEchoEscapes`, lines 555-568 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params, when_shell};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, has_flag, only_literal_string};
use shellcheck_core::interface::Shell;
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkUnusedEchoEscapes",
    registry: "command",
    order: 8,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("echo"), check_unused_echo_escapes),
};

static HAS_ESCAPES: LazyLock<Regex> =
    LazyLock::new(|| mk_regex("\\\\([rntabefv\\\\']|[0-7]{1,3}|x([0-9]|[A-F]|[a-f]){1,2})"));

fn check_unused_echo_escapes(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    when_shell(p, &[Shell::Sh, Shell::Bash, Shell::Ksh], || {
        if !has_flag(p.ast, cmd, "e") {
            for token in arguments(p.ast, cmd) {
                examine(p, token, c);
            }
        }
    });
}

fn examine(p: &Params<'_>, token: Id, c: &mut Comments) {
    let str = only_literal_string(p.ast, token);
    if matches(&str, &HAS_ESCAPES) {
        c.info(token, 2028, "echo may not expand escape sequences. Use printf.");
    }
}
