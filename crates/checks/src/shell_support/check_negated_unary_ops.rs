// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkNegatedUnaryOps`, lines 720-730 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkNegatedUnaryOps",
    registry: "shell-support",
    order: 8,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash], f),
};

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Unary(ConditionType::SingleBracket, o, inner) = p.ast.get(t) {
        if o == "!" {
            if let Token::TC_Unary(_, op, _) = p.ast.get(*inner) {
                if op == "-a" || op == "-o" {
                    c.err(t, 2332, &msg(op));
                }
            }
        }
    }
}

fn msg(op: &str) -> String {
    match op {
        "-o" => "[ ! -o opt ] is always true because -o becomes logical OR. Use [[ ]] or ! [ -o opt ].".to_string(),
        "-a" => "[ ! -a file ] is always true because -a becomes logical AND. Use -e instead.".to_string(),
        // Unreachable given the op guard.
        _ => shellcheck_core::prelude::please_report("unhandled negated unary message"),
    }
}
