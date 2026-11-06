// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkFlagAsCommand`, lines 3820-3826 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::is_unquoted_flag;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkFlagAsCommand",
    registry: "node",
    order: 76,
    optional: None,
    kind: CheckKind::Node(check_flag_as_command),
};

fn check_flag_as_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_SimpleCommand(assigns, words) = p.ast.get(t) {
        if assigns.is_empty() {
            if let Some(&first) = words.first() {
                if is_unquoted_flag(p.ast, first) {
                    c.warn(
                        first,
                        2215,
                        "This flag is used as a command name. Bad line break or missing [ .. ]?",
                    );
                }
            }
        }
    }
}
