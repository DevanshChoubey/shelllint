// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkBadParameterSubstitution`, lines 1885-1945 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::astlib::{
    e4m, is_command_substitution, is_special_variable_char, is_unmodified_parameter_expansion,
    is_variable_char, is_variable_name, is_variable_start_char,
};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkBadParameterSubstitution",
    registry: "node",
    order: 32,
    optional: None,
    kind: CheckKind::Node(check_bad_parameter_substitution),
};

fn check_bad_parameter_substitution(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    let Token::T_DollarBraced(_, word) = ast.get(t) else { return };
    let Token::T_NormalWord(contents) = ast.get(*word) else { return };
    let Some(&first) = contents.first() else { return };
    if is_indirection(ast, contents) {
        c.err(t, 2082, "To expand via indirection, use arrays, ${!name} or (for sh only) eval.");
    } else {
        check_first(ast, first, c);
    }
}

fn is_indirection(ast: &Ast, vars: &[Id]) -> bool {
    let list: Vec<bool> = vars.iter().filter_map(|&v| is_indirection_part(ast, v)).collect();
    !list.is_empty() && list.iter().all(|&b| b)
}

fn is_indirection_part(ast: &Ast, t: Id) -> Option<bool> {
    match ast.get(t) {
        Token::T_DollarExpansion(..) => Some(true),
        Token::T_Backticked(..) => Some(true),
        Token::T_DollarBraced(..) => Some(true),
        Token::T_DollarArithmetic(..) => Some(true),
        Token::T_Literal(s) => {
            if s.chars().all(is_variable_char) {
                None
            } else {
                Some(false)
            }
        }
        _ => Some(false),
    }
}

fn check_first(ast: &Ast, t: Id, c: &mut Comments) {
    match ast.get(t) {
        Token::T_Literal(s) if !s.is_empty() => {
            let ch = s.chars().next().unwrap();
            if is_variable_char(ch) || is_special_variable_char(ch) {
            } else {
                c.err(
                    t,
                    2296,
                    format!("Parameter expansions can't start with {}. Double check syntax.", e4m(&ch.to_string())),
                );
            }
        }
        Token::T_ParamSubSpecialChar(..) => {}
        Token::T_DoubleQuoted(l)
            if l.len() == 1 && matches!(ast.get(l[0]), Token::T_Literal(s) if is_variable(s)) =>
        {
            c.err(t, 2297, "Double quotes must be outside ${}: ${\"invalid\"} vs \"${valid}\".");
        }
        Token::T_DollarBraced(braces, _) if is_unmodified_parameter_expansion(ast, t) => {
            c.err(
                t,
                2298,
                format!(
                    "{} is invalid. For expansion, use ${{x}}. For indirection, use arrays, ${{!x}} or (for sh) eval.",
                    if *braces { "${${x}}" } else { "${$x}" }
                ),
            );
        }
        Token::T_DollarBraced(..) => {
            c.err(t, 2299, "Parameter expansions can't be nested. Use temporary variables.");
        }
        _ if is_command_substitution(ast, t) => {
            c.err(
                t,
                2300,
                "Parameter expansion can't be applied to command substitutions. Use temporary variables.",
            );
        }
        _ => {
            c.err(
                t,
                2301,
                format!("Parameter expansion starts with unexpected {}. Double check syntax.", name(ast, t)),
            );
        }
    }
}

fn is_variable(s: &str) -> bool {
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(ch), None) => is_variable_start_char(ch) || is_special_variable_char(ch) || ch.is_ascii_digit(),
        _ => is_variable_name(s),
    }
}

fn name(ast: &Ast, t: Id) -> &'static str {
    match ast.get(t) {
        Token::T_SingleQuoted(..) => "quotes",
        Token::T_DoubleQuoted(..) => "quotes",
        _ => "syntax",
    }
}
