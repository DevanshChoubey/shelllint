// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkEmptyCondition`, lines 3829-3832 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkEmptyCondition",
    registry: "node",
    order: 77,
    optional: None,
    kind: CheckKind::Node(check_empty_condition),
};

fn check_empty_condition(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Empty(_) = p.ast.get(t) {
        c.style(t, 2212, "Use 'false' instead of empty [/[[ conditionals.");
    }
}
