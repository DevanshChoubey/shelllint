// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkAliasesExpandEarly`, lines 936-944 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_word_parts, is_literal, oversimplify};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkAliasesExpandEarly",
    registry: "command",
    order: 20,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("alias"), check_aliases_expand_early),
};

fn check_aliases_expand_early(p: &Params<'_>, t: Id, c: &mut Comments) {
    for arg in arguments(p.ast, t) {
        check_arg(p, arg, c);
    }
}

fn check_arg(p: &Params<'_>, arg: Id, c: &mut Comments) {
    if oversimplify(p.ast, arg).concat().contains('=') {
        if let Some(x) = get_word_parts(p.ast, arg).into_iter().find(|&x| !is_literal(p.ast, x)) {
            c.warn(x, 2139, "This expands when defined, not when used. Consider escaping.");
        }
    }
}
