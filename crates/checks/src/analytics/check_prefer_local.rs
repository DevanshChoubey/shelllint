// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: no Haskell counterpart. shelllint's own optional check
// `prefer-local` (SC9002, issue #468).

use std::collections::{BTreeMap, BTreeSet};

use shelllint_analyzer::analyzerlib::{Params, StackData, is_true_assignment_source, make_comment};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_command_name, get_path};
use shelllint_core::data::INTERNAL_VARIABLES;
use shelllint_core::interface::{Severity, Shell, TokenComment};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPreferLocal",
    registry: "optional-tree",
    order: 11,
    optional: Some(StaticCheckDescription {
        name: "prefer-local",
        id: &[9002],
        description: "Suggest 'local' for function variables that are never read outside the function",
        positive: "f() { x=1; echo \"$x\"; }",
        negative: "f() { local x=1; echo \"$x\"; }",
    }),
    kind: CheckKind::Tree(check_prefer_local),
};

/// The innermost function containing `t`, if any.
fn enclosing_function(p: &Params<'_>, t: Id) -> Option<Id> {
    get_path(p.ast, t).into_iter().find(|&x| matches!(p.ast.get(x), Token::T_Function(..)))
}

fn check_prefer_local(p: &Params<'_>, _root: Id) -> Vec<TokenComment> {
    // Plain sh has no `local`.
    if p.shell_type == Shell::Sh {
        return vec![];
    }
    let ast = p.ast;

    // Per (function, variable): the first plain assignment, and whether the
    // variable is declared local, exported or made readonly in that function.
    let mut first_assignment: BTreeMap<(Id, String), Id> = BTreeMap::new();
    let mut exempt: BTreeSet<(Id, String)> = BTreeSet::new();
    // Every read: (variable, reading token).
    let mut reads: Vec<(&str, Id)> = Vec::new();

    for s in &p.variable_flow {
        match s {
            StackData::Assignment(base, token, name, source) => {
                let Some(f) = enclosing_function(p, *token) else { continue };
                let key = (f, name.clone());
                let command = match ast.get(*base) {
                    Token::T_SimpleCommand(..) => get_command_name(ast, *base),
                    _ => None,
                };
                match command.as_deref() {
                    // `declare`/`typeset` are local too, unless -g, which says global on purpose.
                    Some("local" | "export" | "readonly" | "declare" | "typeset") => {
                        exempt.insert(key);
                    }
                    _ => {
                        if is_true_assignment_source(source) {
                            first_assignment.entry(key).or_insert(*token);
                        }
                    }
                }
            }
            StackData::Reference(base, token, name) => {
                // `export x` / `readonly x` in a function expose the variable.
                if let Token::T_SimpleCommand(..) = ast.get(*base) {
                    if matches!(get_command_name(ast, *base).as_deref(), Some("export" | "readonly")) {
                        if let Some(f) = enclosing_function(p, *token) {
                            exempt.insert((f, name.clone()));
                        }
                    }
                }
                reads.push((name.as_str(), *token));
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for ((f, name), token) in &first_assignment {
        if exempt.contains(&(*f, name.clone())) || INTERNAL_VARIABLES.contains(&name.as_str()) {
            continue;
        }
        let read_outside = reads
            .iter()
            .any(|(n, r)| *n == name.as_str() && !get_path(ast, *r).contains(f));
        if read_outside {
            continue;
        }
        out.push(make_comment(
            Severity::StyleC,
            *token,
            9002,
            format!("{name} is assigned in a function but never read outside it. Declare it with 'local'."),
        ));
    }
    out
}
