// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRedirectedNowhere`, lines 3608-3632 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_path;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRedirectedNowhere",
    registry: "node",
    order: 70,
    optional: None,
    kind: CheckKind::Node(check_redirected_nowhere),
};

fn check_redirected_nowhere(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, list) = p.ast.get(t) else { return };
    if let &[single] = &list[..] {
        if let Some(redir) = get_dangling_redirect(p, single) {
            if !is_in_expansion(p, t) {
                c.warn(
                    redir,
                    2188,
                    "This redirection doesn't have a command. Move to its command (or use 'true' as no-op).",
                );
            }
        }
    } else {
        for &x in list {
            if let Some(redir) = get_dangling_redirect(p, x) {
                c.err(
                    redir,
                    2189,
                    "You can't have | between this redirection and the command it should apply to.",
                );
            }
        }
    }
}

fn is_in_expansion(p: &Params<'_>, t: Id) -> bool {
    let path = get_path(p.ast, t);
    match path.get(1) {
        Some(&x) => match p.ast.get(x) {
            Token::T_DollarExpansion(l) if l.len() == 1 => true,
            Token::T_Backticked(l) if l.len() == 1 => true,
            Token::T_Annotation(..) => is_in_expansion(p, x),
            _ => false,
        },
        None => false,
    }
}

fn get_dangling_redirect(p: &Params<'_>, token: Id) -> Option<Id> {
    match p.ast.get(token) {
        Token::T_Redirecting(redirs, cmd) => match p.ast.get(*cmd) {
            Token::T_SimpleCommand(a, b) if a.is_empty() && b.is_empty() => redirs.first().copied(),
            _ => None,
        },
        _ => None,
    }
}
