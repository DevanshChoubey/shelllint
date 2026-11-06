// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSecondArgIsComparison`, lines 4796-4820 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_leading_unquoted_string;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSecondArgIsComparison",
    registry: "node",
    order: 90,
    optional: None,
    kind: CheckKind::Node(check_second_arg_is_comparison),
};

fn check_second_arg_is_comparison(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let [_lhs, arg, ..] = &words[..] else { return };
    let arg = *arg;
    let Some(arg_string) = get_leading_unquoted_string(p.ast, arg) else { return };
    if arg_string.starts_with("====") {
        // Don't warn about `echo ======` and such
    } else if arg_string.starts_with("+=") {
        c.err(head_id(p, t), 2285, "Remove spaces around += to assign (or quote '+=' if literal).");
    } else if arg_string.starts_with("==") {
        c.err(t, 2284, "Use [ x = y ] to compare values (or quote '==' if literal).");
    } else if arg_string.starts_with('=') {
        c.err(
            head_id(p, arg),
            2283,
            "Remove spaces around = to assign (or use [ ] to compare, or quote '=' if literal).",
        );
    }
}

// We don't pinpoint exactly, but this helps cases like foo =$bar
fn head_id(p: &Params<'_>, t: Id) -> Id {
    match p.ast.get(t) {
        Token::T_NormalWord(l) if !l.is_empty() => l[0],
        _ => t,
    }
}
