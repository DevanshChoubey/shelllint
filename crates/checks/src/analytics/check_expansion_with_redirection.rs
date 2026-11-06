// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkExpansionWithRedirection`, lines 5415-5442 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkExpansionWithRedirection",
    registry: "node",
    order: 101,
    optional: None,
    kind: CheckKind::Node(check_expansion_with_redirection),
};

fn check_expansion_with_redirection(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_DollarExpansion(l) => {
            if let &[cmd] = &l[..] {
                check(p, c, t, cmd);
            }
        }
        Token::T_Backticked(l) => {
            if let &[cmd] = &l[..] {
                check(p, c, t, cmd);
            }
        }
        Token::T_DollarBraceCommandExpansion(_, l) => {
            if let &[cmd] = &l[..] {
                check(p, c, t, cmd);
            }
        }
        _ => {}
    }
}

fn check(p: &Params<'_>, c: &mut Comments, id: Id, pipe: Id) {
    if let Token::T_Pipeline(_, cmds) = p.ast.get(pipe) {
        if let Some(&last) = cmds.last() {
            check_cmd(p, c, id, last);
        }
    }
}

fn check_cmd(p: &Params<'_>, c: &mut Comments, capture_id: Id, cmd: Id) {
    let Token::T_Redirecting(redirs, _) = p.ast.get(cmd) else { return };
    // foldr (walk captureId) (return ()) redirs: the first redirect that
    // matches a case decides; non-matching ones defer to the rest.
    for &r in redirs {
        if walk(p, c, capture_id, r) {
            return;
        }
    }
}

/// Returns true if the redirect matched one of the cases (so the fold stops).
fn walk(p: &Params<'_>, c: &mut Comments, capture_id: Id, t: Id) -> bool {
    let Token::T_FdRedirect(str, inner) = p.ast.get(t) else { return false };
    match p.ast.get(*inner) {
        Token::T_IoDuplicate(_, s) if s == "1" => return true,
        Token::T_IoDuplicate(..) if str == "1" => return true,
        Token::T_IoDuplicate(op, _) if str.is_empty() => {
            if matches!(p.ast.get(*op), Token::T_GREATAND | Token::T_Greater) {
                emit(c, t, capture_id, true);
                return true;
            }
        }
        Token::T_IoFile(op, file) if str.is_empty() || str == "1" => {
            if matches!(p.ast.get(*op), Token::T_DGREAT | Token::T_Greater) {
                emit(
                    c,
                    t,
                    capture_id,
                    get_literal_string(p.ast, *file).as_deref() != Some("/dev/null"),
                );
                return true;
            }
        }
        _ => {}
    }
    false
}

fn emit(c: &mut Comments, redirect_id: Id, capture_id: Id, suggest_tee: bool) {
    c.warn(
        capture_id,
        2327,
        "This command substitution will be empty because the command's output gets redirected away.",
    );
    c.err(
        redirect_id,
        2328,
        format!(
            "This redirection takes output away from the command substitution{}",
            if suggest_tee { " (use tee to duplicate)." } else { "." }
        ),
    );
}
