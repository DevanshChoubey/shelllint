// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkLoopKeywordScope`, lines 2952-2971 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, Scope, get_path_m, lead_type};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{get_command_name, is_function, is_loop};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkLoopKeywordScope",
    registry: "node",
    order: 50,
    optional: None,
    kind: CheckKind::Node(check_loop_keyword_scope),
};

fn check_loop_keyword_scope(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Some(name) = get_command_name(params.ast, t) else { return };
    if name != "continue" && name != "break" {
        return;
    }
    let subshell_type = |t: Id| -> Option<String> {
        match lead_type(params, t) {
            Scope::SubshellScope(s) => Some(s),
            _ => None,
        }
    };
    let relevant = |t: Id| is_loop(params.ast, t) || is_function(params.ast, t) || subshell_type(t).is_some();
    let path: Vec<Id> = get_path_m(params, t).into_iter().filter(|&x| relevant(x)).collect();
    if path.iter().any(|&x| is_loop(params.ast, x)) {
        let first = path.iter().filter(|&&x| !is_function(params.ast, x)).map(|&x| subshell_type(x)).next();
        if let Some(Some(s)) = first {
            c.warn(t, 2106, format!("This only exits the subshell caused by the {}.", s));
        }
    } else {
        // breaking at a source/function invocation is an abomination. Let's ignore it.
        match path.first() {
            Some(&h) if is_function(params.ast, h) => {
                c.err(t, 2104, format!("In functions, use return instead of {}.", name))
            }
            _ => c.err(t, 2105, format!("{} is only valid in loops.", name)),
        }
    }
}
