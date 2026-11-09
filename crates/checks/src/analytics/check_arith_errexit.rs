// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own optional check `arith-errexit` (SC9003), issue #2777.
// There is no Haskell counterpart.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::astlib::get_path;
use shelllint_core::interface::TokenComment;

use crate::support::run_node_analysis;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArithErrexit",
    registry: "optional-tree",
    order: 100,
    optional: Some(StaticCheckDescription {
        name: "arith-errexit",
        id: &[9003],
        description: "Notify when a standalone ((..)) command can exit with status 1 under set -e",
        positive: "set -e; a=0; ((a++))",
        negative: "set -e; a=0; ((a++)) || true; if ((a++)); then :; fi; ((a = 5))",
    }),
    kind: CheckKind::Tree(check_arith_errexit_tree),
};

fn check_arith_errexit_tree(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    run_node_analysis(&check_arith_errexit, p, t)
}

fn check_arith_errexit(p: &Params<'_>, t: Id, c: &mut Comments) {
    if !p.has_errexit {
        return;
    }
    let Token::T_Arithmetic(body) = p.ast.get(t) else { return };
    if !can_be_zero(p.ast, *body) {
        return;
    }
    if !is_standalone(p.ast, t) {
        return;
    }
    c.warn(
        t,
        9003,
        "This arithmetic command exits with status 1 when its value is 0, which ends the script under set -e. Append '|| true', or use a=$((a+1)) instead.",
    );
}

/// Whether the command's exit status could end the script: it is not a
/// condition, not the left side of `&&`/`||`, not negated and not part of
/// a multi-command pipeline or a substitution.
fn is_standalone(ast: &Ast, t: Id) -> bool {
    for w in get_path(ast, t).windows(2) {
        let (child, parent) = (w[0], w[1]);
        let exempt = match ast.get(parent) {
            Token::T_AndIf(l, _) | Token::T_OrIf(l, _) => *l == child,
            Token::T_Banged(_) => true,
            Token::T_IfExpression(conds, _) => conds.iter().any(|(cond, _)| cond.contains(&child)),
            Token::T_WhileExpression(cond, _) | Token::T_UntilExpression(cond, _) => cond.contains(&child),
            Token::T_Pipeline(_, cmds) => cmds.len() > 1,
            Token::T_DollarExpansion(_) | Token::T_Backticked(_) | Token::T_ProcSub(..) => true,
            _ => false,
        };
        if exempt {
            return false;
        }
    }
    true
}

/// False only when the value is provably non-zero.
fn can_be_zero(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::TA_Sequence(l) => l.last().is_none_or(|&x| can_be_zero(ast, x)),
        Token::TA_Parenthesis(x) => can_be_zero(ast, *x),
        Token::TA_Assignment(op, _, rhs) if op == "=" => can_be_zero(ast, *rhs),
        Token::TA_Expansion(parts) => match parts.as_slice() {
            [lit] => match ast.get(*lit) {
                Token::T_Literal(s) => !matches!(parse_int(s), Some(n) if n != 0),
                _ => true,
            },
            _ => true,
        },
        _ => true,
    }
}

fn parse_int(s: &str) -> Option<u128> {
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u128::from_str_radix(h, 16).ok();
    }
    if s.len() > 1 && s.starts_with('0') {
        return u128::from_str_radix(&s[1..], 8).ok();
    }
    s.parse().ok()
}
