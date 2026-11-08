// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkFgrepSendsPipefail`, lines 432-434 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::Id;

use crate::support::check_grep_sends_pipefail_impl;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkFgrepSendsPipefail",
    registry: "command",
    order: 44,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("fgrep"), check_fgrep_sends_pipefail),
};

fn check_fgrep_sends_pipefail(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_grep_sends_pipefail_impl(p, t, c)
}
