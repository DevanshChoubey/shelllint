// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTildeInPath`, lines 3131-3144 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::astlib::only_literal_string;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTildeInPath",
    registry: "node",
    order: 64,
    optional: None,
    kind: CheckKind::Node(check_tilde_in_path),
};

fn check_tilde_in_path(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(vars, _) = p.ast.get(t) else { return };
    for &v in vars {
        check_var(p, v, c);
    }
}

fn check_var(p: &Params<'_>, id: Id, c: &mut Comments) {
    let Token::T_Assignment(AssignmentMode::Assign, name, indices, word) = p.ast.get(id) else {
        return;
    };
    if name != "PATH" || !indices.is_empty() {
        return;
    }
    let Token::T_NormalWord(parts) = p.ast.get(*word) else { return };
    if parts.iter().any(|&x| is_quoted(p, x) && has_tilde(p, x)) {
        c.warn(id, 2147, "Literal tilde in PATH works poorly across programs.");
    }
}

fn has_tilde(p: &Params<'_>, t: Id) -> bool {
    only_literal_string(p.ast, t).contains('~')
}

fn is_quoted(p: &Params<'_>, t: Id) -> bool {
    matches!(p.ast.get(t), Token::T_DoubleQuoted(_) | Token::T_SingleQuoted(_))
}
