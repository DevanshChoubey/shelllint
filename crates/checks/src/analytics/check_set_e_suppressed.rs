// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSetESuppressed`, lines 5041-5082 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, contains_errexit};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_path, get_unquoted_literal};
use shellcheck_core::interface::TokenComment;

use crate::support::{functions, run_node_analysis};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSetESuppressed",
    registry: "optional-tree",
    order: 8,
    optional: Some(StaticCheckDescription {
        name: "check-set-e-suppressed",
        id: &[2310, 2311],
        description: "Notify when set -e is suppressed during function invocation",
        positive: "set -e; func() { cp *.txt ~/backup; rm *.txt; }; func && echo ok; x=$(func)",
        negative: "set -e; func() { cp *.txt ~/backup; rm *.txt; }; func; echo ok; x=$(set -e; func)",
    }),
    kind: CheckKind::Tree(check_set_e_suppressed),
};

fn check_set_e_suppressed(params: &Params<'_>, t: Id) -> Vec<TokenComment> {
    if !params.has_errexit {
        return vec![];
    }
    let functions_ = functions(params.ast, t);

    let check_node = |p: &Params<'_>, x: Id, c: &mut Comments| {
        if let Token::T_SimpleCommand(_, words) = p.ast.get(x) {
            if let Some(&cmd) = words.first() {
                if is_function(p, &functions_, cmd) {
                    check_cmd(p, cmd, c);
                }
            }
        }
    };
    run_node_analysis(&check_node, params, t)
}

fn is_function(
    p: &Params<'_>,
    functions_: &std::collections::BTreeMap<String, Id>,
    cmd: Id,
) -> bool {
    match get_unquoted_literal(p.ast, cmd) {
        Some(lit) => functions_.contains_key(&lit),
        None => false,
    }
}

fn check_cmd(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    let path = get_path(p.ast, cmd);
    // `go (child:parent:rest)`: every adjacent pair along the path.
    for w in path.windows(2) {
        let (child, parent) = (w[0], w[1]);
        match p.ast.get(parent) {
            Token::T_Banged(condition) if child == *condition => {
                inform_conditional(c, "a ! condition", cmd)
            }
            Token::T_AndIf(condition, _) if child == *condition => {
                inform_conditional(c, "an && condition", cmd)
            }
            Token::T_OrIf(condition, _) if child == *condition => {
                inform_conditional(c, "an || condition", cmd)
            }
            Token::T_IfExpression(conds, _) if conds.iter().any(|(cond, _)| cond.contains(&child)) => {
                inform_conditional(c, "an 'if' condition", cmd)
            }
            Token::T_UntilExpression(condition, _) if condition.contains(&child) => {
                inform_conditional(c, "an 'until' condition", cmd)
            }
            Token::T_WhileExpression(condition, _) if condition.contains(&child) => {
                inform_conditional(c, "a 'while' condition", cmd)
            }
            Token::T_DollarExpansion(_) | Token::T_Backticked(_)
                if !err_exit_enabled(p, parent) =>
            {
                inform_uninherited(c, cmd)
            }
            _ => {}
        }
    }
}

fn inform_conditional(c: &mut Comments, cond_type: &str, t: Id) {
    c.info(
        t,
        2310,
        format!(
            "This function is invoked in {cond_type} so set -e will be disabled. Invoke separately if failures should cause the script to exit."
        ),
    );
}

fn inform_uninherited(c: &mut Comments, t: Id) {
    c.info(
        t,
        2311,
        "Bash implicitly disabled set -e for this function invocation because it's inside a command substitution. Add set -e; before it or enable inherit_errexit.",
    );
}

fn err_exit_enabled(p: &Params<'_>, t: Id) -> bool {
    p.has_inherit_errexit || contains_errexit(p.ast, t)
}
