// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkGrepSendsPipefail`, lines 426-427 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::check_grep_sends_pipefail_impl;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkGrepSendsPipefail",
    registry: "command",
    order: 42,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("grep"), check_grep_sends_pipefail),
};

fn check_grep_sends_pipefail(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_grep_sends_pipefail_impl(p, t, c)
}
