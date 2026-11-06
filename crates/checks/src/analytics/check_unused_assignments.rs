// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnusedAssignments`, lines 2567-2589 (e11f964)

use std::collections::{BTreeMap, BTreeSet};

use shellcheck_analyzer::analyzerlib::{Comments, Params, StackData};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_literal_string, is_variable_char, is_variable_name};
use shellcheck_core::data::INTERNAL_VARIABLES;
use shellcheck_core::interface::TokenComment;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnusedAssignments",
    registry: "tree",
    order: 4,
    optional: None,
    kind: CheckKind::Tree(check_unused_assignments),
};

fn check_unused_assignments(params: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let ast = params.ast;
    let flow = &params.variable_flow;

    let strip_suffix = |name: &str| -> String { name.chars().take_while(|c| is_variable_char(*c)).collect() };

    let mut references: BTreeSet<String> = BTreeSet::new();
    for f in flow {
        if let StackData::Reference(_base, _token, name) = f {
            references.insert(strip_suffix(name));
        }
    }
    for v in INTERNAL_VARIABLES {
        references.insert((*v).to_string());
    }

    // Variables set by bats' run (or pretended to be set by @test)
    // are for the caller to check, if needed
    let is_bats = |t: Id| -> bool {
        match ast.get(t) {
            Token::T_SimpleCommand(_, cmd) => match cmd.first() {
                Some(c) => get_literal_string(ast, *c).as_deref() == Some("run"),
                None => false,
            },
            Token::T_BatsTest(..) => true,
            _ => false,
        }
    };

    let mut assignments: BTreeMap<String, Id> = BTreeMap::new();
    for f in flow {
        if let StackData::Assignment(base, token, name, _) = f {
            if is_variable_name(name) && !is_bats(*base) {
                assignments.insert(name.clone(), *token);
            }
        }
    }

    let mut comments = Comments::new();
    for (name, token) in &assignments {
        if references.contains(name) {
            continue;
        }
        if name.starts_with('_') || params.has_allexport {
            continue;
        }
        comments.warn(
            *token,
            2034,
            format!("{} appears unused. Verify use (or export if used externally).", name),
        );
    }
    comments.into_vec()
}
