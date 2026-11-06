// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkAssignAteCommand`, lines 509-525 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_unquoted_literal, is_glob, is_unquoted_flag};
use shellcheck_core::data::COMMON_COMMANDS;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkAssignAteCommand",
    registry: "node",
    order: 24,
    optional: None,
    kind: CheckKind::Node(check_assign_ate_command),
};

fn check_assign_ate_command(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(assigns, list) = p.ast.get(t) else { return };
    let &[a] = &assigns[..] else { return };
    let Token::T_Assignment(_, _, _, assignment_term) = p.ast.get(a) else { return };
    let id = t;
    let is_common_command = |s: Option<String>| match s {
        Some(s) => COMMON_COMMANDS.contains(&s.as_str()),
        None => false,
    };
    let first_word_is_arg = match list.first() {
        Some(&head) => is_glob(p.ast, head) || is_unquoted_flag(p.ast, head),
        None => false,
    };
    // Check if first word is intended as an argument (flag or glob).
    if first_word_is_arg {
        c.err(id, 2037, "To assign the output of a command, use var=$(cmd) .");
    } else if is_common_command(get_unquoted_literal(p.ast, *assignment_term)) {
        // Check if it's a known, unquoted command name.
        c.warn(id, 2209, "Use var=$(command) to assign output (or quote to assign string).");
    }
}
