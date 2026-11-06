// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFindNameGlob`, lines 259-268 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string, is_glob};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFindNameGlob",
    registry: "command",
    order: 1,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_find_name_glob),
};

fn accepts_glob(s: &str) -> bool {
    [
        "-ilname",
        "-iname",
        "-ipath",
        "-iregex",
        "-iwholename",
        "-lname",
        "-name",
        "-path",
        "-regex",
        "-wholename",
    ]
    .contains(&s)
}

fn check_find_name_glob(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    // f (x:xs) = foldr g (const $ return ()) xs x : visits consecutive pairs (a, b) in order.
    for pair in args.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if let Some(s) = get_literal_string(p.ast, a) {
            if accepts_glob(&s) && is_glob(p.ast, b) {
                c.warn(b, 2061, &format!("Quote the parameter to {} so the shell won't interpret it.", s));
            }
        }
    }
}
