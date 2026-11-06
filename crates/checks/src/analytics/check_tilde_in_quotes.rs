// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTildeInQuotes`, lines 2007-2016 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTildeInQuotes",
    registry: "node",
    order: 36,
    optional: None,
    kind: CheckKind::Node(check_tilde_in_quotes),
};

fn verify(c: &mut Comments, id: Id, s: &str) {
    if s.starts_with("~/") {
        c.warn(id, 2088, "Tilde does not expand in quotes. Use $HOME.");
    }
}

fn check_tilde_in_quotes(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(t) else { return };
    let Some(&first) = parts.first() else { return };
    match p.ast.get(first) {
        Token::T_SingleQuoted(s) => verify(c, first, s),
        Token::T_DoubleQuoted(inner) => {
            if let Some(&lit) = inner.first() {
                if let Token::T_Literal(s) = p.ast.get(lit) {
                    verify(c, lit, s);
                }
            }
        }
        _ => {}
    }
}
