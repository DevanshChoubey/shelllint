// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCpLegacyR`, lines 3372-3380 (e11f964)

use crate::support::{fix_with, replace_token};
use shelllint_analyzer::analyzerlib::{Comments, Params, is_unqualified_command};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_all_flags;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCpLegacyR",
    registry: "node",
    order: 66,
    optional: None,
    kind: CheckKind::Node(check_cp_legacy_r),
};

fn check_cp_legacy_r(params: &Params<'_>, t: Id, c: &mut Comments) {
    if !matches!(params.ast.get(t), Token::T_SimpleCommand(..)) {
        return;
    }
    if !is_unqualified_command(params.ast, t, "cp") {
        return;
    }
    let flags = get_all_flags(params.ast, t);
    let legacy_flag = flags.iter().find(|(_, s)| s == "r");
    if let Some(&(id, _)) = legacy_flag {
        c.warn_with_fix(
            id,
            2336,
            "cp -r behavior is implementation-defined",
            fix_with(vec![replace_token(id, params, "-R")]),
        );
    }
}
