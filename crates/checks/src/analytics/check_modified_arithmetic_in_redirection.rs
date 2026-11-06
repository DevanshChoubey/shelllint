// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkModifiedArithmeticInRedirection`, lines 4371-4399 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_word_parts;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkModifiedArithmeticInRedirection",
    registry: "node",
    order: 85,
    optional: None,
    kind: CheckKind::Node(check_modified_arithmetic_in_redirection),
};

fn check_modified_arithmetic_in_redirection(p: &Params<'_>, t: Id, c: &mut Comments) {
    if p.shell_type == Shell::Dash || p.shell_type == Shell::BusyboxSh {
        return;
    }
    if let Token::T_Redirecting(redirs, cmd) = p.ast.get(t) {
        if let Token::T_SimpleCommand(_, words) = p.ast.get(*cmd) {
            if !words.is_empty() {
                for &r in redirs {
                    check_redirs(p, r, c);
                }
            }
        }
    }
}

fn check_redirs(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_FdRedirect(_, inner) = p.ast.get(t) else { return };
    match p.ast.get(*inner) {
        Token::T_IoFile(_, word) | Token::T_HereString(word) => {
            for part in get_word_parts(p.ast, *word) {
                check_arithmetic(p, part, c);
            }
        }
        Token::T_HereDoc(_, _, _, list) => {
            for &part in list {
                check_arithmetic(p, part, c);
            }
        }
        _ => {}
    }
}

fn check_arithmetic(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_DollarArithmetic(x) = p.ast.get(t) {
        check_modifying(p, *x, c);
    }
}

fn check_modifying(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::TA_Sequence(list) => {
            for &x in list {
                check_modifying(p, x, c);
            }
        }
        Token::TA_Unary(s, _) if ["|++", "++|", "|--", "--|"].contains(&s.as_str()) => {
            warn_for(t, c)
        }
        Token::TA_Assignment(..) => warn_for(t, c),
        Token::TA_Binary(_, x, y) => {
            check_modifying(p, *x, c);
            check_modifying(p, *y, c);
        }
        Token::TA_Trinary(x, y, z) => {
            check_modifying(p, *x, c);
            check_modifying(p, *y, c);
            check_modifying(p, *z, c);
        }
        _ => {}
    }
}

fn warn_for(id: Id, c: &mut Comments) {
    c.warn(
        id,
        2257,
        "Arithmetic modifications in command redirections may be discarded. Do them separately.",
    );
}
