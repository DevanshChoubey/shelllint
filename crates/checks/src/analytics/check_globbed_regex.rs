// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkGlobbedRegex`, lines 1454-1460 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, is_confused_glob_regex};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{ConditionType, Id, Token};
use shelllint_core::astlib::oversimplify;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkGlobbedRegex",
    registry: "node",
    order: 30,
    optional: None,
    kind: CheckKind::Node(check_globbed_regex),
};

fn check_globbed_regex(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Binary(ConditionType::DoubleBracket, op, _, rhs) = p.ast.get(t) else { return };
    if op != "=~" {
        return;
    }
    let s: String = oversimplify(p.ast, *rhs).concat();
    if is_confused_glob_regex(&s) {
        c.warn(*rhs, 2049, "=~ is for regex, but this looks like a glob. Use = instead.");
    }
}
