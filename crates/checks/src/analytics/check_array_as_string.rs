// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArrayAsString`, lines 1005-1015 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{will_become_multiple_args, will_concat_in_assignment};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArrayAsString",
    registry: "node",
    order: 57,
    optional: None,
    kind: CheckKind::Node(check_array_as_string),
};

fn check_array_as_string(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(_, _, _, word) = p.ast.get(t) else { return };
    let word = *word;
    if will_concat_in_assignment(p.ast, word) {
        c.warn(
            word,
            2124,
            "Assigning an array to a string! Assign as array, or use * instead of @ to concatenate.",
        );
    } else if will_become_multiple_args(p.ast, word) {
        c.warn(
            word,
            2125,
            "Brace expansions and globs are literal in assignments. Quote it or use an array.",
        );
    }
}
