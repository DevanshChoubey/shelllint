// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFindWithoutPath`, lines 965-982 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_literal_string_def, has_flag};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFindWithoutPath",
    registry: "command",
    order: 22,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_find_without_path),
};

fn check_find_without_path(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let Some((&cmd, args)) = words.split_first() else { return };
    if !(has_flag(p.ast, t, "help") || has_path(p, args)) {
        c.info(cmd, 2185, "Some finds don't have a default path. Specify '.' explicitly.");
    }
}

// This is a bit of a kludge. find supports flag arguments both before and
// after the path, as well as multiple non-flag arguments that are not the
// path. We assume that all the pre-path flags are single characters from a
// list of GNU and macOS flags.
fn has_path(p: &Params<'_>, args: &[Id]) -> bool {
    match args.split_first() {
        Some((&first, rest)) => {
            let flag = get_literal_string_def(p.ast, "___", first);
            !flag.starts_with('-') || is_leading_flag(&flag) && has_path(p, rest)
        }
        None => false,
    }
}

fn is_leading_flag(flag: &str) -> bool {
    flag.chars().count() <= 2 || flag.chars().all(|ch| LEADING_FLAG_CHARS.contains(ch))
}

const LEADING_FLAG_CHARS: &str = "-EHLPXdfsxO0123456789";
