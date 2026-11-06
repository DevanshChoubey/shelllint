// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRedirectionToNumber`, lines 3799-3805 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_unquoted_literal;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRedirectionToNumber",
    registry: "node",
    order: 74,
    optional: None,
    kind: CheckKind::Node(check_redirection_to_number),
};

fn check_redirection_to_number(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_IoFile(_, word) = p.ast.get(t) else { return };
    let Some(file) = get_unquoted_literal(p.ast, *word) else { return };
    if file.chars().all(|ch| ch.is_ascii_digit()) {
        c.warn(
            t,
            2210,
            "This is a file redirection. Was it supposed to be a comparison or fd operation?",
        );
    }
}
