// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCommandIsUnreachable`, lines 5246-5272 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_analyzer::cfg_analysis::get_incoming_state;
use shelllint_core::ast::{Id, Token};
use shelllint_analyzer::analyzerlib::p_script;
use shelllint_core::ast::Ast;
use shelllint_core::astlib::{get_command_argv, get_command_name, get_literal_string, get_path};

use crate::support::is_sourced;
use crate::zsh_support::zsh_registered_functions;
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCommandIsUnreachable",
    registry: "node",
    order: 95,
    optional: None,
    kind: CheckKind::Node(check_command_is_unreachable),
};

fn check_command_is_unreachable(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Pipeline(..) => {
            let Some(cfga) = p.cfg_analysis.as_deref() else { return };
            let Some(state) = get_incoming_state(cfga, t) else { return };
            if state.state_is_reachable() {
                return;
            }
            if is_sourced(p, t) {
                return;
            }
            if get_path(p.ast, t).into_iter().skip(1).any(|t| is_unreachable(p, t) || is_unreachable_function(p, t)) {
                return;
            }
            c.info(t, 2317, "Command appears to be unreachable. Check usage (or ignore if invoked indirectly).");
        }
        Token::T_Function(..) => {
            if is_unreachable_function(p, t)
                && !get_path(p.ast, t).into_iter().skip(1).any(|t| is_unreachable_function(p, t))
                && !is_sourced(p, t)
                // DIVERGENCE: a function named in a `trap` action is invoked when the trap fires, so it is
                // not reported as never invoked.
                && !is_trapped_function(p.ast, p.root, t)
                // DIVERGENCE: zsh calls hook, widget, completion and `TRAPxxx` functions itself.
                && !is_zsh_registered(p, t)
            {
                c.info(t, 2329, "This function is never invoked. Check usage (or ignored if invoked indirectly).");
            }
        }
        _ => {}
    }
}

fn is_zsh_registered(p: &Params<'_>, f: Id) -> bool {
    let Token::T_Function(_, _, name, _) = p.ast.get(f) else { return false };
    p.shell_type == Shell::Zsh && zsh_registered_functions(p).contains(name)
}

fn is_unreachable_function(p: &Params<'_>, f: Id) -> bool {
    match p.ast.get(f) {
        Token::T_Function(_, _, _, t) => is_unreachable(p, *t),
        _ => false,
    }
}

fn is_unreachable(p: &Params<'_>, t: Id) -> bool {
    let Some(cfga) = p.cfg_analysis.as_deref() else { return false };
    let Some(state) = get_incoming_state(cfga, t) else { return false };
    !state.state_is_reachable()
}

/// Whether the name of function `f` appears as a command in the first argument of some `trap`.
fn is_trapped_function(ast: &Ast, root: Id, f: Id) -> bool {
    let Token::T_Function(_, _, name, _) = ast.get(f) else { return false };
    reachable_ids(ast, root).into_iter().any(|t| trap_action_commands(ast, t).iter().any(|c| c == name))
}

/// For a `trap` command, the names of the commands in its action when that is a literal; else empty.
fn trap_action_commands(ast: &Ast, t: Id) -> Vec<String> {
    let Token::T_SimpleCommand(..) = ast.get(t) else { return vec![] };
    if get_command_name(ast, t).as_deref() != Some("trap") {
        return vec![];
    }
    let Some(argv) = get_command_argv(ast, t) else { return vec![] };
    let skip_dd = argv.get(1).and_then(|&a| get_literal_string(ast, a)).as_deref() == Some("--");
    let Some(action) = argv.get(1 + usize::from(skip_dd)).and_then(|&a| get_literal_string(ast, a)) else {
        return vec![];
    };
    let pr = p_script(&action);
    let Some(proot) = pr.root else { return vec![] };
    reachable_ids(&pr.ast, proot)
        .into_iter()
        .filter(|&c| matches!(pr.ast.get(c), Token::T_SimpleCommand(..)))
        .filter_map(|c| get_command_name(&pr.ast, c))
        .collect()
}

/// Every token reachable from `root`, parents before children.
fn reachable_ids(ast: &Ast, root: Id) -> Vec<Id> {
    let mut out = vec![];
    let mut stack = vec![root];
    while let Some(t) = stack.pop() {
        out.push(t);
        stack.extend(ast.children(t));
    }
    out
}
