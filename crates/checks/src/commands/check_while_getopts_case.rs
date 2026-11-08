// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkWhileGetoptsCase`, lines 1068-1142 (e11f964)

use std::collections::{BTreeMap, BTreeSet};

use shelllint_analyzer::analyzerlib::{Comments, Params, find_first, get_path_m, modifies_variable};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::{CaseType, Id, Token};
use shelllint_core::astlib::{e4m, get_literal_string, get_word_parts};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkWhileGetoptsCase",
    registry: "command",
    order: 29,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("getopts"), check_while_getopts_case),
};

type CaseList = Vec<(CaseType, Vec<Id>, Vec<Id>)>;

fn check_while_getopts_case(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    let Token::T_SimpleCommand(_, words) = ast.get(t) else { return };
    if words.len() < 3 {
        return;
    }
    let (arg1, name) = (words[1], words[2]);
    let path = get_path_m(p, t);
    let Some(options) = get_literal_string(ast, arg1) else { return };
    let Some(getopts_var) = get_literal_string(ast, name) else { return };
    let while_loop = |x: &Id| -> Option<bool> {
        match ast.get(*x) {
            Token::T_WhileExpression(..) => Some(true),
            Token::T_Script(..) => Some(false),
            _ => None,
        }
    };
    let Some(&wl) = find_first(while_loop, &path) else { return };
    let Token::T_WhileExpression(_, body) = ast.get(wl) else { return };
    let Some(case_id) = body.iter().filter_map(|&x| find_case(p, x)).next() else { return };
    let Token::T_CaseExpression(var, list) = ast.get(case_id) else { return };

    // Make sure getopts name and case variable matches
    let parts = get_word_parts(ast, *var);
    let [one] = parts.as_slice() else { return };
    let Token::T_DollarBraced(_, braced_word) = ast.get(*one) else { return };
    let bparts = get_word_parts(ast, *braced_word);
    let [lit] = bparts.as_slice() else { return };
    let Token::T_Literal(case_var) = ast.get(*lit) else { return };
    if *case_var != getopts_var {
        return;
    }

    // Make sure the variable isn't modified
    // Haskell wraps the body in a synthetic T_BraceGroup; the variable flow
    // of a brace group is just that of its children, so none is built.
    if body.iter().any(|&b| modifies_variable(p, b, &getopts_var)) {
        return;
    }

    let opts: Vec<String> = options.chars().filter(|&ch| ch != ':').map(|ch| ch.to_string()).collect();
    check(p, c, &opts, case_id, list);
}

fn check(p: &Params<'_>, c: &mut Comments, opts: &[String], id: Id, list: &CaseList) {
    // M.fromList: later entries overwrite earlier ones
    let mut handled_map: BTreeMap<Option<String>, Id> = BTreeMap::new();
    for (_, globs, _) in list {
        for &g in globs {
            handled_map.insert(literal(p, g), g);
        }
    }
    let requested_map: BTreeSet<Option<String>> = opts.iter().map(|x| Some(x.clone())).collect();

    let not_handled = requested_map.iter().filter(|k| !handled_map.contains_key(*k));
    let not_requested = handled_map.iter().filter(|(k, _)| !requested_map.contains(*k));

    if !handled_map.contains_key(&None) {
        for k in not_handled.flatten() {
            warn_unhandled(c, id, k);
        }
        if ![Some("*".to_string()), Some("?".to_string())].iter().any(|k| handled_map.contains_key(k)) {
            c.warn(id, 2220, "Invalid flags are not handled. Add a *) case.");
        }
    }

    for (k, expr) in not_requested {
        warn_redundant(c, k, *expr);
    }
}

fn warn_unhandled(c: &mut Comments, case_id: Id, str: &str) {
    c.warn(
        case_id,
        2213,
        format!("getopts specified -{}, but it's not handled by this 'case'.", e4m(str)),
    );
}

fn warn_redundant(c: &mut Comments, key: &Option<String>, expr: Id) {
    if let Some(str) = key {
        if !["*", ":", "?"].contains(&str.as_str()) {
            c.warn(expr, 2214, "This case is not specified by getopts.");
        }
    }
}

fn literal(p: &Params<'_>, t: Id) -> Option<String> {
    // Maybe's Semigroup: concatenate when both are Just
    match (get_literal_string(p.ast, t), from_glob(p, t)) {
        (Some(a), Some(b)) => Some(a + &b),
        (a, b) => a.or(b),
    }
}

fn from_glob(p: &Params<'_>, t: Id) -> Option<String> {
    match p.ast.get(t) {
        Token::T_Glob(s) => {
            let cs: Vec<char> = s.chars().collect();
            match cs.as_slice() {
                ['[', ch, ']'] => Some(ch.to_string()),
                ['*'] => Some("*".to_string()),
                ['?'] => Some("?".to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn find_case(p: &Params<'_>, t: Id) -> Option<Id> {
    match p.ast.get(t) {
        Token::T_Annotation(_, x) => find_case(p, *x),
        Token::T_Pipeline(_, l) if l.len() == 1 => find_case(p, l[0]),
        Token::T_Redirecting(_, x) if matches!(p.ast.get(*x), Token::T_CaseExpression(..)) => Some(*x),
        _ => None,
    }
}
