// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCommarrays`, lines 1651-1662 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCommarrays",
    registry: "node",
    order: 18,
    optional: None,
    kind: CheckKind::Node(check_commarrays),
};

fn check_commarrays(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Array(l) = p.ast.get(t) else { return };
    if l.iter().any(|&x| is_comma_separated(&literal(p.ast, x))) {
        c.warn(t, 2054, "Use spaces, not commas, to separate array elements.");
    }
}

fn literal(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_IndexedElement(_, l) => literal(ast, *l),
        Token::T_NormalWord(l) => l.iter().map(|&x| literal(ast, x)).collect(),
        Token::T_Literal(s) => s.clone(),
        _ => String::new(),
    }
}

fn is_comma_separated(s: &str) -> bool {
    s.contains(',')
}
