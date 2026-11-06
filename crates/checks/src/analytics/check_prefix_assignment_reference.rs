// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPrefixAssignmentReference`, lines 2849-2868 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_reference, get_path, oversimplify};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPrefixAssignmentReference",
    registry: "node",
    order: 49,
    optional: None,
    kind: CheckKind::Node(check_prefix_assignment_reference),
};

fn check_prefix_assignment_reference(p: &Params<'_>, id: Id, c: &mut Comments) {
    let Token::T_DollarBraced(_, value) = p.ast.get(id) else { return };
    let name = get_braced_reference(&oversimplify(p.ast, *value).concat());
    let path = get_path(p.ast, id);
    let id_path = &path;

    for &t in &path {
        if let Token::T_SimpleCommand(vars, words) = p.ast.get(t) {
            if !words.is_empty() {
                for &v in vars {
                    check_var(p, v, &name, id_path, id, c);
                }
                return;
            }
        }
    }
}

fn check_var(p: &Params<'_>, a_id: Id, name: &str, id_path: &[Id], id: Id, c: &mut Comments) {
    if let Token::T_Assignment(_mode, a_name, indices, _value) = p.ast.get(a_id) {
        if indices.is_empty() && a_name == name && !id_path.contains(&a_id) {
            c.warn(a_id, 2097, "This assignment is only seen by the forked process.");
            c.warn(id, 2098, "This expansion will not see the mentioned assignment.");
        }
    }
}
