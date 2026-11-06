// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArithmeticDeref`, lines 1575-1593 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, parents};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArithmeticDeref",
    registry: "node",
    order: 14,
    optional: None,
    kind: CheckKind::Node(check_arithmetic_deref),
};

fn is_exception(s: &str) -> bool {
    let Some(h) = s.chars().next() else { return true };
    s.chars().any(|c| "/.:#%?*@$-!+=^,".contains(c)) || h.is_ascii_digit()
}

/// Some(true) = warn, Some(false) = no warning, None = keep looking.
fn warning_for(p: &Params<'_>, t: Id) -> Option<bool> {
    match p.ast.get(t) {
        Token::T_Arithmetic(..)
        | Token::T_DollarArithmetic(..)
        | Token::T_ForArithmetic(..)
        | Token::T_Assignment(..) => Some(true),
        Token::T_SimpleCommand(..) => Some(false),
        _ => None,
    }
}

fn check_arithmetic_deref(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TA_Expansion(l) = params.ast.get(t) else { return };
    let [id] = l.as_slice() else { return };
    let Token::T_DollarBraced(_, w) = params.ast.get(*id) else { return };
    if is_exception(&oversimplify(params.ast, *w).concat()) {
        return;
    }
    let warn = parents(params, t)
        .into_iter()
        .find_map(|x| warning_for(params, x))
        .unwrap_or(false);
    if warn {
        c.style(*id, 2004, "$/${} is unnecessary on arithmetic variables.");
    }
}
