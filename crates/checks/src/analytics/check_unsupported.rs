// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnsupported`, lines 3149-3158 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{CaseType, Id, Token};
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnsupported",
    registry: "node",
    order: 58,
    optional: None,
    kind: CheckKind::Node(check_unsupported),
};

fn check_unsupported(p: &Params<'_>, t: Id, c: &mut Comments) {
    let (name, support) = shell_support(p, t);
    if !(support.is_empty() || support.contains(&p.shell_type)) {
        // report
        let shells: Vec<String> = support
            .iter()
            .map(|s| format!("{:?}", s).to_lowercase())
            .collect();
        c.err(
            t,
            2127,
            format!("To use {}, specify #!/usr/bin/env {}", name, shells.join(" or ")),
        );
    }
}

fn shell_support(p: &Params<'_>, t: Id) -> (&'static str, Vec<Shell>) {
    match p.ast.get(t) {
        Token::T_CaseExpression(_, list) => {
            let seps: Vec<CaseType> = list.iter().map(|(a, _, _)| *a).collect();
            if seps.contains(&CaseType::CaseContinue) {
                ("cases with ;;&", vec![Shell::Bash])
            } else if seps.contains(&CaseType::CaseFallThrough) {
                ("cases with ;&", vec![Shell::Bash, Shell::Ksh])
            } else {
                ("", vec![])
            }
        }
        Token::T_DollarBraceCommandExpansion(..) => {
            ("${ ..; } command expansion", vec![Shell::Bash, Shell::Ksh])
        }
        _ => ("", vec![]),
    }
}
