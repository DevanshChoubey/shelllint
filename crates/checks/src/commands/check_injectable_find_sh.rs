// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkInjectableFindSh`, lines 572-593 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, only_literal_string};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkInjectableFindSh",
    registry: "command",
    order: 9,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_injectable_find_sh),
};

type Pred = fn(&str) -> bool;

/// `CommandCheck (Basename "find") (check . arguments)`
fn check_injectable_find_sh(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    let id_strings: Vec<(Id, String)> =
        args.iter().map(|&x| (x, only_literal_string(p.ast, x))).collect();
    let pattern: [Pred; 3] = [
        |s| ["-exec", "-execdir", "-ok", "-okdir"].contains(&s),
        |s| ["sh", "bash", "dash", "ksh"].contains(&s),
        |s| s == "-c",
    ];
    match_pat(&pattern, &id_strings, c);
}

fn match_pat(pat: &[Pred], list: &[(Id, String)], c: &mut Comments) {
    if list.is_empty() {
        return;
    }
    let Some((p, tests)) = pat.split_first() else {
        action(&list[0], c);
        return;
    };
    let (_, arg) = &list[0];
    if p(arg) {
        match_pat(tests, &list[1..], c);
    }
    match_pat(pat, &list[1..], c);
}

fn action((id, arg): &(Id, String), c: &mut Comments) {
    if arg.contains("{}") {
        c.warn(*id, 2156, "Injecting filenames is fragile and insecure. Use parameters.");
    }
}
