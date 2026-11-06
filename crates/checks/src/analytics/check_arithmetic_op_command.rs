// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArithmeticOpCommand`, lines 529-537 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_glob_or_literal_string;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArithmeticOpCommand",
    registry: "node",
    order: 44,
    optional: None,
    kind: CheckKind::Node(check_arithmetic_op_command),
};

fn check_arithmetic_op_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(assigns, words) = p.ast.get(t) else { return };
    let &[a] = &assigns[..] else { return };
    if !matches!(p.ast.get(a), Token::T_Assignment(..)) {
        return;
    }
    let Some(&first_word) = words.first() else { return };
    if let Some(op) = get_glob_or_literal_string(p.ast, first_word) {
        if ["+", "-", "*", "/"].contains(&op.as_str()) {
            c.warn(
                first_word,
                2099,
                &format!("Use $((..)) for arithmetics, e.g. i=$((i {} 2))", op),
            );
        }
    }
}
