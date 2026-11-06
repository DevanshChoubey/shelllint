// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkEgrepSendsPipefail`, lines 429-430 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;

use crate::support::check_grep_sends_pipefail_impl;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkEgrepSendsPipefail",
    registry: "command",
    order: 43,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("egrep"), check_egrep_sends_pipefail),
};

fn check_egrep_sends_pipefail(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_grep_sends_pipefail_impl(p, t, c)
}
