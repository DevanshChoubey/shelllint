// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkInjectableFindSh`, lines 572-593 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, only_literal_string};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkInjectableFindSh",
    registry: "command",
    order: 9,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("find"), check_injectable_find_sh),
};

/// `CommandCheck (Basename "find") (check . arguments)`
fn check_injectable_find_sh(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    let id_strings: Vec<(Id, String)> =
        args.iter().map(|&x| (x, only_literal_string(p.ast, x))).collect();
    match_pattern(&id_strings, c);
}

// `match pattern idStrings`, where
//   pattern = [(`elem` ["-exec", "-execdir", "-ok", "-okdir"]),
//              (`elem` ["sh", "bash", "dash", "ksh"]),
//              (== "-c")]
// and
//   match _ [] = return ()
//   match [] (next:_) = action next
//   match (p:tests) ((id, arg):args) = do
//       when (p arg) $ match tests args
//       match (p:tests) args
//
// The backtracking `match` runs `action` once for every ordered
// (-exec, sh, -c) triple, i.e. on `list[k]` once per pair i < j < k-1 with
// pattern[0] at i, pattern[1] at j and pattern[2] at k-1. Its duplicate
// comments are identical and dropped later by `nubTokenComments`, but there
// are O(n^3) of them. This scan visits each `k` once, in the same order as
// the first time the backtracking search reaches it, and acts on it if at
// least one such triple exists: the same comments after the nub, in linear
// time.
fn match_pattern(list: &[(Id, String)], c: &mut Comments) {
    let is_exec = |s: &str| ["-exec", "-execdir", "-ok", "-okdir"].contains(&s);
    let is_shell = |s: &str| ["sh", "bash", "dash", "ksh"].contains(&s);
    // `seen_exec`: some pattern[0] at an index < the current one;
    // `seen_shell`: some pattern[0] .. pattern[1] pair before it.
    let (mut seen_exec, mut seen_shell) = (false, false);
    for (k, (_, arg)) in list.iter().enumerate() {
        if seen_shell && arg == "-c"
            && let Some(next) = list.get(k + 1)
        {
            action(next, c);
        }
        if seen_exec && is_shell(arg) {
            seen_shell = true;
        }
        if is_exec(arg) {
            seen_exec = true;
        }
    }
}

fn action((id, arg): &(Id, String), c: &mut Comments) {
    if arg.contains("{}") {
        c.warn(*id, 2156, "Injecting filenames is fragile and insecure. Use parameters.");
    }
}
