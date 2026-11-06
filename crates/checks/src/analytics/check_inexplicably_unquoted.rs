// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkInexplicablyUnquoted`, lines 1958-2001 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::get_path;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkInexplicablyUnquoted",
    registry: "node",
    order: 35,
    optional: None,
    kind: CheckKind::Node(check_inexplicably_unquoted),
};

fn check_inexplicably_unquoted(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_NormalWord(tokens) = p.ast.get(t) else { return };
    for i in 0..tokens.len() {
        check(p, &tokens[i..], c);
    }
}

fn check(p: &Params<'_>, toks: &[Id], c: &mut Comments) {
    let ast = p.ast;
    if let [a, b, ..] = toks {
        if let (Token::T_SingleQuoted(_), Token::T_Literal(s)) = (ast.get(*a), ast.get(*b)) {
            if !s.is_empty() && s.chars().all(|ch| ch.is_alphanumeric()) {
                c.info(
                    *b,
                    2026,
                    "This word is outside of quotes. Did you intend to 'nest '\"'single quotes'\"' instead'? ",
                );
                return;
            }
        }
    }

    if let [a, trapped, b, ..] = toks {
        if let (Token::T_DoubleQuoted(la), Token::T_DoubleQuoted(lb)) = (ast.get(*a), ast.get(*b)) {
            let trapped = *trapped;
            match ast.get(trapped) {
                Token::T_DollarExpansion(_) | Token::T_DollarBraced(..) => {
                    warn_about_expansion(trapped, c)
                }
                Token::T_Literal(s) => {
                    if !(quotes_single_thing(ast, la) && quotes_single_thing(ast, lb)
                        || s == "="
                        || s == ":"
                        || s == "/"
                        || is_special(ast, &get_path(ast, trapped)))
                    {
                        warn_about_literal(trapped, c);
                    }
                }
                _ => {}
            }
        }
    }
}

// Regexes for [[ .. =~ re ]] are parsed with metacharacters like ()| as unquoted
// literals. The same is true for ${x+"foo" "bar"}. Avoid overtriggering on these.
fn is_special(ast: &Ast, path: &[Id]) -> bool {
    match path {
        [first, ..] if matches!(ast.get(*first), Token::T_Redirecting(..)) => false,
        [first, ..] if matches!(ast.get(*first), Token::T_DollarBraced(..)) => true,
        [a, second, ..]
            if matches!(ast.get(*second), Token::TC_Binary(_, op, _, _) if op == "=~") =>
        {
            let Token::TC_Binary(_, _, _, rhs) = ast.get(*second) else { return false };
            a == rhs
        }
        [_, rest @ ..] => is_special(ast, rest),
        [] => false,
    }
}

// If the surrounding quotes quote single things, like "$foo"_and_then_some_"$stuff",
// the quotes were probably intentional and harmless.
fn quotes_single_thing(ast: &Ast, x: &[Id]) -> bool {
    match x {
        [one] => matches!(
            ast.get(*one),
            Token::T_DollarExpansion(_) | Token::T_DollarBraced(..) | Token::T_Backticked(_)
        ),
        _ => false,
    }
}

fn warn_about_expansion(id: Id, c: &mut Comments) {
    c.warn(id, 2027, "The surrounding quotes actually unquote this. Remove or escape them.");
}

fn warn_about_literal(id: Id, c: &mut Comments) {
    c.warn(
        id,
        2140,
        "Word is of the form \"A\"B\"C\" (B indicated). Did you mean \"ABC\" or \"A\\\"B\\\"C\"?",
    );
}
