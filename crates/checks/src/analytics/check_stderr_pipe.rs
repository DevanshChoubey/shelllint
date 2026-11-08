// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkStderrPipe`, lines 2998-3006 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkStderrPipe",
    registry: "node",
    order: 55,
    optional: None,
    kind: CheckKind::Node(check_stderr_pipe),
};

fn check_stderr_pipe(params: &Params<'_>, t: Id, c: &mut Comments) {
    if params.shell_type == Shell::Ksh { match_(params, t, c) }
}

fn match_(p: &Params<'_>, id: Id, c: &mut Comments) {
    if let Token::T_Pipe(s) = p.ast.get(id) {
        if s == "|&" {
            c.err(id, 2118, "Ksh does not support |&. Use 2>&1 |.");
        }
    }
}
