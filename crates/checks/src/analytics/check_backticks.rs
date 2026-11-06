// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkBackticks`, lines 1866-1872 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};

use crate::support::{fix_with, replace_end, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkBackticks",
    registry: "node",
    order: 34,
    optional: None,
    kind: CheckKind::Node(check_backticks),
};

fn check_backticks(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Backticked(list) = params.ast.get(t) else { return };
    if list.is_empty() {
        return;
    }
    c.style_with_fix(
        t,
        2006,
        "Use $(...) notation instead of legacy backticks `...`.",
        fix_with(vec![replace_start(t, params, 1, "$("), replace_end(t, params, 1, ")")]),
    );
}
