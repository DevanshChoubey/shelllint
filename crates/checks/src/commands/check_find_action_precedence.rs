// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFindActionPrecedence`, lines 597-612 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::Id;
use shelllint_core::astlib::{arguments, get_literal_string};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFindActionPrecedence",
    registry: "command",
    order: 10,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_find_action_precedence),
};

/// `f . arguments`
fn check_find_action_precedence(p: &Params<'_>, t: Id, c: &mut Comments) {
    let list = arguments(p.ast, t);
    f(p, &list, c);
}

const PATTERN_LEN: usize = 6;

fn pattern_matches(p: &Params<'_>, list: &[Id]) -> bool {
    // pattern = [isMatch, const True, isParam ["-o", "-or"], isMatch, const True, isAction]
    is_match(p, list[0])
        && is_param(p, &["-o", "-or"], list[2])
        && is_match(p, list[3])
        && is_action(p, list[5])
}

fn f(p: &Params<'_>, list: &[Id], c: &mut Comments) {
    let mut list = list;
    loop {
        if list.len() < PATTERN_LEN {
            return;
        }
        if pattern_matches(p, list) {
            warn_for(list[PATTERN_LEN - 1], c);
            return;
        }
        list = &list[1..];
    }
}

fn is_match(p: &Params<'_>, t: Id) -> bool {
    is_param(p, &["-name", "-regex", "-iname", "-iregex", "-wholename", "-iwholename"], t)
}

fn is_action(p: &Params<'_>, t: Id) -> bool {
    is_param(
        p,
        &[
            "-exec", "-execdir", "-delete", "-print", "-print0", "-fls", "-fprint", "-fprint0",
            "-fprintf", "-ls", "-ok", "-okdir", "-printf",
        ],
        t,
    )
}

fn is_param(p: &Params<'_>, strs: &[&str], t: Id) -> bool {
    match get_literal_string(p.ast, t) {
        Some(param) => strs.contains(&param.as_str()),
        None => false,
    }
}

fn warn_for(t: Id, c: &mut Comments) {
    c.warn(t, 2146, "This action ignores everything before the -o. Use \\( \\) to group.");
}
