// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRequireDoubleEquals`, lines 4901-4911 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{
    CheckEntry, CheckKind, StaticCheckDescription, node_checks_to_tree_check,
};
use shelllint_core::ast::{Id, Token};
use shelllint_core::interface::{Shell, TokenComment};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRequireDoubleEquals",
    registry: "optional-tree",
    order: 7,
    optional: Some(StaticCheckDescription {
        name: "require-double-equals",
        id: &[2338],
        description: "Require == and warn about = in Bash tests",
        positive: "[[ \"$x\" = \"$y\" ]]",
        negative: "[[ \"$x\" == \"$y\" ]]",
    }),
    kind: CheckKind::Tree(check_require_double_equals),
};

fn check_require_double_equals(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    if p.shell_type == Shell::Bash {
        node_checks_to_tree_check(p, t, &[check])
    } else {
        vec![]
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Binary(_, op, _, _) = p.ast.get(t)
        && op == "="
    {
        c.style(t, 2338, "Prefer == over = for string comparisons in Bash tests.");
    }
}
