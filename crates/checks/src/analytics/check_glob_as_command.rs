// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkGlobAsCommand`, lines 3809-3815 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::is_glob;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkGlobAsCommand",
    registry: "node",
    order: 75,
    optional: None,
    kind: CheckKind::Node(check_glob_as_command),
};

fn check_glob_as_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let Some(&first) = words.first() else { return };
    if is_glob(p.ast, first) {
        c.warn(
            first,
            2211,
            "This is a glob used as a command name. Was it supposed to be in ${..}, array, or is it missing quoting?",
        );
    }
}
