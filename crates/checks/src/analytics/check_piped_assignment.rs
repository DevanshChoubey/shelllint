// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPipedAssignment`, lines 498-501 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPipedAssignment",
    registry: "node",
    order: 23,
    optional: None,
    kind: CheckKind::Node(check_piped_assignment),
};

fn check_piped_assignment(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, cmds) = p.ast.get(t) else { return };
    if cmds.len() < 2 {
        return;
    }
    let Token::T_Redirecting(_, cmd) = p.ast.get(cmds[0]) else { return };
    let Token::T_SimpleCommand(assigns, words) = p.ast.get(*cmd) else { return };
    if !assigns.is_empty() && words.is_empty() {
        c.warn(*cmd, 2036, "If you wanted to assign the output of the pipeline, use a=$(b | c) .");
    }
}
