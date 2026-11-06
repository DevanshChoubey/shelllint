// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkLonelyDotDash`, lines 2019-2024 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_unqualified_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkLonelyDotDash",
    registry: "node",
    order: 37,
    optional: None,
    kind: CheckKind::Node(check_lonely_dot_dash),
};

fn check_lonely_dot_dash(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Redirecting(..) = p.ast.get(t) {
        if is_unqualified_command(p.ast, t, "./") {
            c.err(t, 2083, "Don't add spaces after the slash in './file'.");
        }
    }
}
