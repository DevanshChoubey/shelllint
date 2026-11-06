// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkAliasesUsesArgs`, lines 922-932 (e11f964)

use std::sync::LazyLock;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string_def};
use regex::Regex;
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkAliasesUsesArgs",
    registry: "command",
    order: 19,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("alias"), check_aliases_uses_args),
};

static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("\\$\\{?[0-9*@]"));

fn check_aliases_uses_args(p: &Params<'_>, t: Id, c: &mut Comments) {
    for arg in arguments(p.ast, t) {
        check_arg(p, arg, c);
    }
}

fn check_arg(p: &Params<'_>, arg: Id, c: &mut Comments) {
    let string = get_literal_string_def(p.ast, "_", arg);
    if string.contains('=') && matches(&string, &RE) {
        c.err(arg, 2142, "Aliases can't use positional parameters. Use a function.");
    }
}
