// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkWrongArithmeticAssignment`, lines 540-561 (e11f964)

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use shellcheck_analyzer::analyzerlib::{Comments, Params, StackData};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::regex::{match_regex, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkWrongArithmeticAssignment",
    registry: "node",
    order: 52,
    optional: None,
    kind: CheckKind::Node(check_wrong_arithmetic_assignment),
};

static REGEX: LazyLock<Regex> =
    LazyLock::new(|| mk_regex("^([_a-zA-Z][_a-zA-Z0-9]*)([+*-]).+$"));

fn check_wrong_arithmetic_assignment(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(assigns, words) = p.ast.get(t) else { return };
    let &[a] = &assigns[..] else { return };
    if !words.is_empty() {
        return;
    }
    let Token::T_Assignment(_, _, _, val) = p.ast.get(a) else { return };
    let val = *val;

    let Some(s) = get_normal_string(p.ast, val) else { return };
    let Some(m) = match_regex(&REGEX, &s) else { return };
    let [var, op, ..] = &m[..] else { return };
    let references: HashSet<&str> = p
        .variable_flow
        .iter()
        .filter_map(|d| match d {
            StackData::Assignment(_, _, name, _) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    if !references.contains(var.as_str()) {
        return;
    }
    c.warn(
        val,
        2100,
        format!("Use $((..)) for arithmetics, e.g. i=$((i {} 2))", op),
    );
}

fn get_normal_string(ast: &Ast, val: Id) -> Option<String> {
    let Token::T_NormalWord(words) = ast.get(val) else { return None };
    let mut out = String::new();
    for &w in words {
        out.push_str(get_literals(ast, w)?);
    }
    Some(out)
}

fn get_literals(ast: &Ast, w: Id) -> Option<&str> {
    match ast.get(w) {
        Token::T_Literal(s) => Some(s),
        Token::T_Glob(s) => Some(s),
        _ => None,
    }
}
