// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkForDecimals`, lines 1543-1549 (e11f964)
//
// Not to be confused with Checks/ShellSupport.hs `checkForDecimals`, which
// is the one ShellCheck runs: this one is in no registration list (dead code
// upstream) but has prop_ tests, so it is registered under "unregistered",
// which `run_check` reaches and the module checkers never run.

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;

use crate::support::has_floating_point;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkForDecimals",
    registry: "unregistered",
    order: 0,
    optional: None,
    kind: CheckKind::Node(check_for_decimals),
};

fn check_for_decimals(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TA_Expansion(_) = params.ast.get(t) else { return };
    if has_floating_point(params) {
        return;
    }
    let Some(s) = get_literal_string(params.ast, t) else { return };
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return };
    if first.is_ascii_digit() && chars.as_str().contains('.') {
        c.err(t, 2079, "(( )) doesn't support decimals. Use bc or awk.");
    }
}
