// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkTrapQuotes`, lines 470-482 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::arguments;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkTrapQuotes",
    registry: "command",
    order: 4,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("trap"), check_trap_quotes),
};

fn check_trap_quotes(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Some(&x) = arguments(p.ast, t).first() {
        check_trap(p, x, c);
    }
}

fn check_trap(p: &Params<'_>, x: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(x) else { return };
    let [q] = &parts[..] else { return };
    let Token::T_DoubleQuoted(rs) = p.ast.get(*q) else { return };
    for &r in rs {
        check_expansions(p, r, c);
    }
}

fn warning(id: Id, c: &mut Comments) {
    c.warn(id, 2064, "Use single quotes, otherwise this expands now rather than when signalled.");
}

fn check_expansions(p: &Params<'_>, id: Id, c: &mut Comments) {
    match p.ast.get(id) {
        Token::T_DollarExpansion(_)
        | Token::T_Backticked(_)
        | Token::T_DollarBraced(..)
        | Token::T_DollarArithmetic(_) => warning(id, c),
        _ => {}
    }
}
