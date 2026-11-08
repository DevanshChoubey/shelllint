// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkAssignToSelf`, lines 4589-4605 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{AssignmentMode, Id, Token};
use shelllint_core::astlib::{get_literal_string, get_word_parts};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkAssignToSelf",
    registry: "node",
    order: 88,
    optional: None,
    kind: CheckKind::Node(check_assign_to_self),
};

fn check_assign_to_self(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(vars, cmd) = p.ast.get(t) else { return };
    if !cmd.is_empty() {
        return;
    }
    for &v in vars {
        check(p, v, c);
    }
}

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(AssignmentMode::Assign, name, idx, val) = p.ast.get(t) else { return };
    if !idx.is_empty() {
        return;
    }
    let parts = get_word_parts(p.ast, *val);
    let &[part] = &parts[..] else { return };
    let Token::T_DollarBraced(_, b) = p.ast.get(part) else { return };
    if get_literal_string(p.ast, *b).as_deref() == Some(name.as_str()) {
        msg(t, c);
    }
}

fn msg(id: Id, c: &mut Comments) {
    c.info(id, 2269, "This variable is assigned to itself, so the assignment does nothing.");
}
