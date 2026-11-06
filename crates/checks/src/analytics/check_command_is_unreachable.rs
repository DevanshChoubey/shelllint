// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCommandIsUnreachable`, lines 5246-5272 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_analyzer::cfg_analysis::get_incoming_state;
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_path;

use crate::support::is_sourced;

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
            {
                c.info(t, 2329, "This function is never invoked. Check usage (or ignored if invoked indirectly).");
            }
        }
        _ => {}
    }
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
