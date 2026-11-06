// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSudoRedirect`, lines 1277-1299 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command_m};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;

use crate::support::parametrised_command_checks;

parametrised_command_checks!("checkSudoRedirect", 50, Basename, check_sudo_redirect_, ["sudo", "doas", "run0"]);

/// `checkSudoRedirect cmd = CommandCheck (Basename cmd) f`
fn check_sudo_redirect_(_cmd: &str, p: &Params<'_>, t: Id, c: &mut Comments) {
    check_sudo_redirect(p, t, c)
}

// The command name only selects the registration; `f` does not use it.
fn check_sudo_redirect(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Some(r) = get_closest_command_m(p, t) {
        if let Token::T_Redirecting(redirs, _) = p.ast.get(r) {
            for &redir in redirs {
                warn_about(p, redir, c);
            }
        }
    }
}

fn warn_about(p: &Params<'_>, redir: Id, c: &mut Comments) {
    let Token::T_FdRedirect(s, io) = p.ast.get(redir) else { return };
    let Token::T_IoFile(op, file) = p.ast.get(*io) else { return };
    if (s.is_empty() || s == "&") && !special(p, *file) {
        match p.ast.get(*op) {
            Token::T_Less => c.info(*op, 2024, "sudo/doas/run0 doesn't affect redirects. Use sudo cat file | .."),
            Token::T_Greater => c.warn(*op, 2024, "sudo/doas/run0 doesn't affect redirects. Use ..| sudo tee file"),
            Token::T_DGREAT => c.warn(*op, 2024, "sudo/doas/run0 doesn't affect redirects. Use .. | sudo tee -a file"),
            _ => {}
        }
    }
}

fn special(p: &Params<'_>, file: Id) -> bool {
    oversimplify(p.ast, file).concat() == "/dev/null"
}
