// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSetAssignment`, lines 847-860 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::astlib::{arguments, is_variable_name};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSetAssignment",
    registry: "command",
    order: 17,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("set"), check_set_assignment),
};

fn check_set_assignment(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    let Some((&var, rest)) = args.split_first() else { return };
    let s = literal(p.ast, var);
    if (!rest.is_empty() && is_variable_name(&s)) || is_assignment(&s) {
        c.warn(var, 2121, "To assign a variable, use just 'var=value', no 'set ..'.");
    }
}

fn is_assignment(s: &str) -> bool {
    s.contains('=')
}

fn literal(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_NormalWord(l) => l.iter().map(|x| literal(ast, *x)).collect(),
        Token::T_Literal(s) => s.clone(),
        _ => "*".to_string(),
    }
}
