// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkForInCat`, lines 757-767 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkForInCat",
    registry: "node",
    order: 27,
    optional: None,
    kind: CheckKind::Node(check_for_in_cat),
};

fn check_for_in_cat(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_ForIn(_, words, _) = p.ast.get(t) else { return };
    let &[w] = &words[..] else { return };
    let Token::T_NormalWord(parts) = p.ast.get(w) else { return };
    for &part in parts {
        check_f(p, part, c);
    }
}

fn check_f(p: &Params<'_>, id: Id, c: &mut Comments) {
    let cmds = match p.ast.get(id) {
        Token::T_DollarExpansion(cmds) | Token::T_Backticked(cmds) => cmds,
        _ => return,
    };
    let &[pipeline] = &cmds[..] else { return };
    let Token::T_Pipeline(_, r) = p.ast.get(pipeline) else { return };
    if r.iter().all(|&cmd| is_line_based(p, cmd)) {
        c.info(id, 2013, "To read lines rather than words, pipe/redirect to a 'while read' loop.");
    }
}

fn is_line_based(p: &Params<'_>, cmd: Id) -> bool {
    ["grep", "fgrep", "egrep", "sed", "cat", "awk", "cut", "sort"]
        .iter()
        .any(|s| is_command(p.ast, cmd, s))
}
