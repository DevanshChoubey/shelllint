// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDollarBrackets`, lines 2093-2096 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDollarBrackets",
    registry: "node",
    order: 40,
    optional: None,
    kind: CheckKind::Node(check_dollar_brackets),
};

fn check_dollar_brackets(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_DollarBracket(..) = p.ast.get(t) {
        c.style(t, 2007, "Use $((..)) instead of deprecated $[..]");
    }
}
