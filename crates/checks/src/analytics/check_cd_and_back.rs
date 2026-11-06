// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCdAndBack`, lines 2921-2944 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_command_sequences, oversimplify};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCdAndBack",
    registry: "node",
    order: 51,
    optional: None,
    kind: CheckKind::Node(check_cd_and_back),
};

fn check_cd_and_back(p: &Params<'_>, t: Id, c: &mut Comments) {
    if p.has_errexit {
        return;
    }
    for list in get_command_sequences(p.ast, t) {
        do_list(p, &list, c);
    }
}

fn is_cd_revert(p: &Params<'_>, t: Id) -> bool {
    let s = oversimplify(p.ast, t);
    match &s[..] {
        [_, x] => x == ".." || x == "-",
        _ => false,
    }
}

fn get_candidate(p: &Params<'_>, t: Id) -> Option<Id> {
    match p.ast.get(t) {
        Token::T_Annotation(_, x) => get_candidate(p, *x),
        Token::T_Pipeline(_, cmds) if cmds.len() == 1 && is_command(p.ast, cmds[0], "cd") => {
            Some(cmds[0])
        }
        _ => None,
    }
}

fn find_cd_pair(p: &Params<'_>, list: &[Id]) -> Option<Id> {
    let mut i = 0;
    while i + 1 < list.len() {
        let (a, b) = (list[i], list[i + 1]);
        if is_cd_revert(p, b) && !is_cd_revert(p, a) {
            return Some(b);
        }
        i += 1;
    }
    None
}

fn do_list(p: &Params<'_>, list: &[Id], c: &mut Comments) {
    let cands: Vec<Id> = list.iter().filter_map(|&x| get_candidate(p, x)).collect();
    if let Some(cd) = find_cd_pair(p, &cands) {
        c.info(cd, 2103, "Use a ( subshell ) to avoid having to cd back.");
    }
}
