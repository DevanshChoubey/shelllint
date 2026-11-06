// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSshHereDoc`, lines 2099-2110 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Quoted, Token};
use shellcheck_core::astlib::{e4m, is_constant};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSshHereDoc",
    registry: "node",
    order: 41,
    optional: None,
    kind: CheckKind::Node(check_ssh_here_doc),
};

fn check_ssh_here_doc(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Redirecting(redirs, cmd) = p.ast.get(t) else { return };
    if is_command(p.ast, *cmd, "ssh") {
        for &r in redirs {
            check_here_doc(p, r, c);
        }
    }
}

fn check_here_doc(p: &Params<'_>, r: Id, c: &mut Comments) {
    let Token::T_FdRedirect(_, hd) = p.ast.get(r) else { return };
    let Token::T_HereDoc(_, Quoted::Unquoted, token, tokens) = p.ast.get(*hd) else { return };
    if !tokens.iter().all(|&x| is_constant(p.ast, x)) {
        c.warn(
            *hd,
            2087,
            format!(
                "Quote '{}' to make here document expansions happen on the server side rather than on the client.",
                e4m(token)
            ),
        );
    }
}
