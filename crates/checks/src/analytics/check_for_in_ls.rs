// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkForInLs`, lines 771-786 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkForInLs",
    registry: "node",
    order: 2,
    optional: None,
    kind: CheckKind::Node(check_for_in_ls),
};

fn check_for_in_ls(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_ForIn(_, words, _) = p.ast.get(t) else { return };
    let &[w] = &words[..] else { return };
    let Token::T_NormalWord(parts) = p.ast.get(w) else { return };
    let &[part] = &parts[..] else { return };
    let (id, cmds) = match p.ast.get(part) {
        Token::T_DollarExpansion(l) | Token::T_Backticked(l) => (part, l),
        _ => return,
    };
    let &[x] = &cmds[..] else { return };
    check(p, id, x, c);
}

fn check(p: &Params<'_>, id: Id, x: Id, c: &mut Comments) {
    let o = oversimplify(p.ast, x);
    match o.split_first() {
        Some((first, n)) if first == "ls" => {
            if n.iter().any(|s| s.starts_with('-')) {
                c.warn(id, 2045, "Iterating over ls output is fragile. Use globs.");
            } else {
                c.err(id, 2045, "Iterating over ls output is fragile. Use globs.");
            }
        }
        Some((first, _)) if first == "find" => {
            c.warn(id, 2044, "For loops over find output are fragile. Use find -exec or a while read loop.");
        }
        _ => {}
    }
}
