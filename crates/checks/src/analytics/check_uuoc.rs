// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUuoc`, lines 570-578 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{may_become_multiple_args, only_literal_string};

use crate::support::check_command;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUuoc",
    registry: "optional-tree",
    order: 10,
    optional: Some(StaticCheckDescription {
        name: "useless-use-of-cat",
        id: &[2002],
        description: "Check for Useless Use Of Cat (UUOC)",
        positive: "cat foo | grep bar",
        negative: "grep bar foo",
    }),
    kind: CheckKind::Node(check_uuoc),
};

fn check_uuoc(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, cmds) = p.ast.get(t) else { return };
    if cmds.len() < 2 {
        return;
    }
    let Token::T_Redirecting(_, cmd) = p.ast.get(cmds[0]) else { return };
    check_command(
        p.ast,
        "cat",
        |_, args| {
            if let &[word] = args {
                if !(may_become_multiple_args(p.ast, word) || is_option(p, word)) {
                    c.style(
                        word,
                        2002,
                        "Useless cat. Consider 'cmd < file | ..' or 'cmd file | ..' instead.",
                    );
                }
            }
        },
        *cmd,
    );
}

fn is_option(p: &Params<'_>, word: Id) -> bool {
    only_literal_string(p.ast, word).starts_with('-')
}
