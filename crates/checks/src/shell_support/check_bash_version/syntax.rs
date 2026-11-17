// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Syntax that needs a newer bash: SC9104 `|&`, SC9105 `;&` / `;;&`, SC9106 `coproc`,
// SC9109 `&>>`, SC9113 `{fd}>` and SC9137 `${ cmd; }`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::{CaseType, Id, Token};

use super::{too_new, too_new_as};

pub fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Pipe(op) if op == "|&" => too_new(p, c, t, 9104),
        Token::T_CaseExpression(_, clauses) => {
            // Reported at the clause's first pattern: the terminator has no node of its own.
            for (kind, patterns, _) in clauses {
                let at = patterns.first().copied().unwrap_or(t);
                match kind {
                    CaseType::CaseFallThrough => too_new_as(p, c, at, 9105, Some(";&")),
                    CaseType::CaseContinue => too_new_as(p, c, at, 9105, Some(";;&")),
                    CaseType::CaseBreak => {}
                }
            }
        }
        Token::T_CoProc(..) => too_new(p, c, t, 9106),
        Token::T_FdRedirect(fd, redirect) => {
            let appends_both = fd == "&"
                && matches!(p.ast.get(*redirect), Token::T_IoFile(op, _) if matches!(p.ast.get(*op), Token::T_DGREAT));
            if appends_both {
                too_new(p, c, t, 9109);
            }
            if fd.len() > 2 && fd.starts_with('{') && fd.ends_with('}') {
                too_new(p, c, t, 9113);
            }
        }
        Token::T_DollarBraceCommandExpansion(..) => too_new(p, c, t, 9137),
        _ => {}
    }
}
