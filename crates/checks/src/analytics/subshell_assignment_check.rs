// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `subshellAssignmentCheck`, lines 2150-2154 (e11f964)

use std::collections::HashMap;

use shellcheck_analyzer::analyzerlib::{
    Comments, Params, Scope, StackData, VariableState, is_true_assignment_source,
};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::interface::TokenComment;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "subshellAssignmentCheck",
    registry: "tree",
    order: 0,
    optional: None,
    kind: CheckKind::Tree(subshell_assignment_check),
};

fn subshell_assignment_check(p: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let flow = &p.variable_flow;
    let mut c = Comments::new();
    find_subshelled(p, flow, vec![(SubshellCheckScope::InSubshell("oops".to_string()), vec![])], HashMap::new(), &mut c);
    c.into_vec()
}

enum SubshellCheckScope {
    /// The reason for the subshell
    InSubshell(String),
    /// Variable states outside the function
    InFunction(HashMap<String, VariableState>),
}

/// An assignment as stored in a scope. Haskell keeps the whole
/// `(base, token, name, data)` tuple, but only the token and the name are
/// read when the scope ends.
type Assigned = (Id, String);

type Scopes = Vec<(SubshellCheckScope, Vec<Assigned>)>;

// The Haskell recursion over `flow` is written as a loop. Scope assignment
// lists keep the newest entry last, so they are walked in reverse to match
// the `x:scope` order.
fn find_subshelled(
    p: &Params<'_>,
    flow: &[StackData],
    mut scopes: Scopes,
    mut dead_vars: HashMap<String, VariableState>,
    c: &mut Comments,
) {
    for item in flow {
        match item {
            StackData::Assignment(_, tok, str, data) => {
                if scopes.is_empty() {
                    return;
                }
                if is_true_assignment_source(data) {
                    scopes.last_mut().unwrap().1.push((*tok, str.clone()));
                    dead_vars.insert(str.clone(), VariableState::Alive);
                }
            }
            StackData::Reference(_, read_token, str) => {
                let should_ignore = ["@", "*", "_", "IFS"].contains(&str.as_str());
                // `export foo=bar` references foo, but does not read its old value
                let is_assigned_by = matches!(
                    p.ast.get(*read_token),
                    Token::T_Assignment(AssignmentMode::Assign, name, _, _) if name == str
                );
                if !(should_ignore || is_assigned_by) {
                    if let Some(VariableState::Dead(write_token, reason)) = dead_vars.get(str) {
                        c.info(*write_token, 2030, format!("Modification of {str} is local (to subshell caused by {reason})."));
                        c.info(*read_token, 2031, format!("{str} was modified in a subshell. That change might be lost."));
                    }
                }
            }
            StackData::StackScope(Scope::SubshellScope(reason)) => {
                scopes.push((SubshellCheckScope::InSubshell(reason.clone()), vec![]));
            }
            // A function body runs where the function is called, not where it is defined,
            // so subshells preceding the definition don't affect the variables it reads.
            StackData::StackScope(Scope::FunctionScope) => {
                let outer = std::mem::take(&mut dead_vars);
                scopes.push((SubshellCheckScope::InFunction(outer), vec![]));
            }
            // Unreachable: AnalyzerLib only pushes a StackScope
            // `when (scopeType /= NoneScope)`, so Haskell needs no case for it.
            StackData::StackScope(Scope::NoneScope) => {}
            StackData::StackScopeEnd => match scopes.pop() {
                Some((SubshellCheckScope::InSubshell(reason), scope)) => {
                    for (token, var) in scope.iter().rev() {
                        dead_vars.insert(var.clone(), VariableState::Dead(*token, reason.clone()));
                    }
                }
                // Likewise, assignments in a function body are not made where the function is
                // defined, so don't add them to the enclosing scope. Keep the effects of
                // subshells inside the function though.
                Some((SubshellCheckScope::InFunction(outer), _)) => {
                    for (k, v) in outer {
                        dead_vars.entry(k).or_insert(v);
                    }
                }
                None => return,
            },
        }
    }
}
