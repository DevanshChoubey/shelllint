// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkStderrRedirect`, lines 1069-1090 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_parent_of};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_path;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkStderrRedirect",
    registry: "node",
    order: 6,
    optional: None,
    kind: CheckKind::Node(check_stderr_redirect),
};

fn check_stderr_redirect(params: &Params<'_>, redir: Id, c: &mut Comments) {
    let ast = params.ast;
    let Token::T_Redirecting(redirs, _) = ast.get(redir) else { return };
    let &[r1, r2] = &redirs[..] else { return };
    let Token::T_FdRedirect(fd, dup) = ast.get(r1) else { return };
    if fd != "2" {
        return;
    }
    let Token::T_IoDuplicate(op1, num) = ast.get(*dup) else { return };
    if !matches!(ast.get(*op1), Token::T_GREATAND) || num != "1" {
        return;
    }
    let Token::T_FdRedirect(_, file) = ast.get(r2) else { return };
    let Token::T_IoFile(op, _) = ast.get(*file) else { return };
    let id = r1;

    let uses_output = |t: Id| -> bool {
        match ast.get(t) {
            Token::T_Pipeline(_, list) => {
                list.len() > 1 && !is_parent_of(ast, *list.last().unwrap(), redir)
            }
            Token::T_ProcSub(..) => true,
            Token::T_DollarExpansion(..) => true,
            Token::T_Backticked(..) => true,
            _ => false,
        }
    };
    let is_captured = get_path(ast, redir).into_iter().any(uses_output);

    match ast.get(*op) {
        Token::T_Greater | Token::T_DGREAT => {
            if !is_captured {
                c.warn(
                    id,
                    2069,
                    "To redirect stdout+stderr, 2>&1 must be last (or use '{ cmd > file; } 2>&1' to clarify).",
                );
            }
        }
        _ => {}
    }
}
