// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkEvalArray`, lines 1480-1494 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{arguments, get_braced_modifier, get_word_parts, is_array_expansion, oversimplify};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkEvalArray",
    registry: "command",
    order: 41,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("eval"), check_eval_array),
};

/// `mapM_ check . concatMap getWordParts . arguments`
fn check_eval_array(p: &Params<'_>, t: Id, c: &mut Comments) {
    for a in arguments(p.ast, t) {
        for part in get_word_parts(p.ast, a) {
            check(p, part, c);
        }
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    if is_array_expansion(p.ast, t) {
        if is_escaped(p, t) {
            c.style(t, 2293, "When eval'ing @Q-quoted words, use * rather than @ as the index.");
        } else {
            c.warn(t, 2294, "eval negates the benefit of arrays. Drop eval to preserve whitespace/symbols (or eval as string).");
        }
    }
}

fn is_escaped(p: &Params<'_>, q: Id) -> bool {
    match p.ast.get(q) {
        // Match ${arr[@]@Q} and ${@@Q} and such
        Token::T_DollarBraced(_, l) => get_braced_modifier(&oversimplify(p.ast, *l).concat()).contains('Q'),
        _ => false,
    }
}
