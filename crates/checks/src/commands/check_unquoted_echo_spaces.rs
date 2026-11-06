// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkUnquotedEchoSpaces`, lines 1455-1474 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command_m};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Pos, Token};
use shellcheck_core::astlib::arguments;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkUnquotedEchoSpaces",
    registry: "command",
    order: 40,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("echo"), check),
};

fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    let redir = get_closest_command_m(p, t);
    let positions: Vec<(Pos, Pos)> = args.iter().map(|&a| p.ast.span(a)).collect();
    let Some(r) = redir else { return };
    let Token::T_Redirecting(redir_tokens, _) = p.ast.get(r) else { return };
    let redir_positions: Vec<Pos> = redir_tokens.iter().map(|&x| p.ast.span(x).0).collect();
    let any = positions
        .iter()
        .zip(positions.iter().skip(1))
        .any(|(&a, &b)| has_spaces_between(&redir_positions, a, b));
    if any {
        c.info(t, 2291, "Quote repeated spaces to avoid them collapsing into one.");
    }
}

fn has_spaces_between(redirs: &[Pos], (a, b): (Pos, Pos), (cc, d): (Pos, Pos)) -> bool {
    a.line == d.line
        && (i64::from(cc.column) - i64::from(b.column)) >= 4
        && !redirs.iter().any(|&x| b < x && x < cc)
}
