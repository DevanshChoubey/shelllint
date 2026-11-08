// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkForLoopGlobVariables`, lines 4069-4080 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{is_glob, is_quoteable_expansion};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkForLoopGlobVariables",
    registry: "node",
    order: 79,
    optional: None,
    kind: CheckKind::Node(check_for_loop_glob_variables),
};

fn check_for_loop_glob_variables(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_ForIn(_, words, _) = p.ast.get(t) {
        for &w in words {
            check(p, w, c);
        }
    }
}

fn check(p: &Params<'_>, w: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(w) else { return };
    if parts.iter().any(|&x| is_glob(p.ast, x)) {
        for &x in parts.iter().filter(|&&x| is_quoteable_expansion(p.ast, x)) {
            suggest(x, c);
        }
    }
}

fn suggest(t: Id, c: &mut Comments) {
    c.info(
        t,
        2231,
        "Quote expansions in this for loop glob to prevent wordsplitting, e.g. \"$dir\"/*.txt .",
    );
}
