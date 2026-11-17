// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Owned by L3: variables, += and tests (SC9101 9110 9118 9121 9123 9129 9132).
// Runs once per node, only when a bash target is set. Report with `super::too_new`.
//
// - SC9101 `+=` is `T_Assignment` in `Append` mode (also `declare x+=y`, `x+=(..)`).
// - SC9110 / SC9129 / SC9132 are reads only: `$X`, `${X}`, `${X:-d}` and arithmetic `X`.
//   An assignment (`BASHPID=1`, `(( SRANDOM = 1 ))`) defines the name and does not fire.
// - SC9118 / SC9123 are the `-v` and `-R` unary tests in `[ ]`, `[[ ]]` and `test`.
// - SC9121 is a literal negative subscript on assignment or `unset`: `a[-1]=x`, `unset 'a[-1]'`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::{AssignmentMode, Ast, Id, Token};
use shelllint_core::astlib::{get_braced_reference, get_command_name, get_literal_string, oversimplify};

use super::{too_new, too_new_as};

pub fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    match ast.get(t) {
        Token::T_Assignment(mode, _, indices, _) => {
            if *mode == AssignmentMode::Append {
                too_new(p, c, t, 9101);
            }
            if indices.iter().any(|&i| is_negative_literal(&index_text(ast, i))) {
                too_new(p, c, t, 9121);
            }
        }
        Token::T_DollarBraced(_, word) => {
            let text = oversimplify(ast, *word).concat();
            variable(p, c, t, &get_braced_reference(&text));
        }
        Token::TA_Variable(name, _) => {
            if !is_assignment_target(ast, t) {
                variable(p, c, t, name);
            }
        }
        Token::TC_Unary(_, op, _) => test_operator(p, c, t, op),
        Token::T_SimpleCommand(_, words) => {
            match get_command_name(ast, t).as_deref() {
                Some("test") => {
                    // `test -v x` and `test ! -v x`
                    let negated = words.len() > 2 && get_literal_string(ast, words[1]).as_deref() == Some("!");
                    let at = if negated { 2 } else { 1 };
                    if words.len() > at + 1 {
                        if let Some(op) = get_literal_string(ast, words[at]) {
                            test_operator(p, c, t, &op);
                        }
                    }
                }
                Some("unset") => {
                    for &w in words.iter().skip(1) {
                        // Unquoted `a[-1]` is a glob, so read the text through `oversimplify`.
                        if negative_subscript(&oversimplify(ast, w).concat()) {
                            too_new(p, c, w, 9121);
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// A read of variable `name`.
fn variable(p: &Params<'_>, c: &mut Comments, t: Id, name: &str) {
    match name {
        "BASHPID" => too_new(p, c, t, 9110),
        "EPOCHSECONDS" | "EPOCHREALTIME" | "BASH_ARGV0" => too_new_as(p, c, t, 9129, Some(&format!("${name}"))),
        "SRANDOM" => too_new_as(p, c, t, 9132, Some("$SRANDOM")),
        _ => {}
    }
}

fn test_operator(p: &Params<'_>, c: &mut Comments, t: Id, op: &str) {
    match op {
        "-v" => too_new(p, c, t, 9118),
        "-R" => too_new(p, c, t, 9123),
        _ => {}
    }
}

/// The left-hand side of `(( X = 1 ))`, which defines X instead of reading it.
fn is_assignment_target(ast: &Ast, t: Id) -> bool {
    matches!(ast.parent(t).map(|p| ast.get(p)), Some(Token::TA_Assignment(_, lhs, _)) if *lhs == t)
}

/// The source text of a subscript, with `?` for anything that is not literal.
fn index_text(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_UnparsedIndex(_, s) | Token::T_Literal(s) | Token::T_Glob(s) | Token::T_SingleQuoted(s) => s.clone(),
        Token::T_NormalWord(l) | Token::T_DoubleQuoted(l) | Token::TA_Sequence(l) | Token::TA_Expansion(l) => {
            l.iter().map(|&x| index_text(ast, x)).collect()
        }
        Token::TA_Unary(op, x) => format!("{op}{}", index_text(ast, *x)),
        _ => "?".to_string(),
    }
}

/// `-1`, `-20`: a literal negative number.
fn is_negative_literal(s: &str) -> bool {
    let s = s.trim();
    s.strip_prefix('-').is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
}

/// `name[-1]`, the argument of `unset`.
fn negative_subscript(s: &str) -> bool {
    let Some(rest) = s.strip_suffix(']') else { return false };
    let Some((name, idx)) = rest.split_once('[') else { return false };
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && is_negative_literal(idx)
}
