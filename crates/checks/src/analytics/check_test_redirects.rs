// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTestRedirects`, lines 1823-1839 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTestRedirects",
    registry: "node",
    order: 31,
    optional: None,
    kind: CheckKind::Node(check_test_redirects),
};

fn check_test_redirects(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Redirecting(redirs, cmd) = p.ast.get(t) else { return };
    if !is_command(p.ast, *cmd, "test") {
        return;
    }
    for &r in redirs {
        if suspicious(p, r) {
            c.warn(r, 2065, "This is interpreted as a shell file redirection, not a comparison.");
        }
    }
}

// Ignore redirections of stderr because these are valid for squashing e.g. int errors,
// and >> and similar redirections because these are probably not comparisons.
fn suspicious(p: &Params<'_>, t: Id) -> bool {
    match p.ast.get(t) {
        Token::T_FdRedirect(fd, io) => match p.ast.get(*io) {
            Token::T_IoFile(op, _) => fd != "2" && is_comparison(p, *op),
            _ => false,
        },
        _ => false,
    }
}

fn is_comparison(p: &Params<'_>, t: Id) -> bool {
    matches!(p.ast.get(t), Token::T_Greater | Token::T_Less)
}
