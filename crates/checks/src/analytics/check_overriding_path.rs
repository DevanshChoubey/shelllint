// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkOverridingPath`, lines 3116-3127 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::astlib::{is_literal, oversimplify};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkOverridingPath",
    registry: "node",
    order: 56,
    optional: None,
    kind: CheckKind::Node(check_overriding_path),
};

fn check_overriding_path(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(vars, rest) = p.ast.get(t) else { return };
    if !rest.is_empty() {
        return;
    }
    for &v in vars {
        check_var(p, v, c);
    }
}

fn check_var(p: &Params<'_>, id: Id, c: &mut Comments) {
    let Token::T_Assignment(AssignmentMode::Assign, name, idx, word) = p.ast.get(id) else { return };
    if name != "PATH" || !idx.is_empty() {
        return;
    }
    let string: String = oversimplify(p.ast, *word).concat();
    if ["/bin", "/sbin"].iter().any(|s| string.contains(s)) {
        return;
    }
    let notify = |c: &mut Comments| c.warn(id, 2123, "PATH is the shell search path. Use another name.");
    if string.contains('/') && !string.contains(':') {
        notify(c);
    }
    if is_literal(p.ast, *word) && !string.contains(':') && !string.contains('/') {
        notify(c);
    }
}
