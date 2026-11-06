// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkEchoWc`, lines 483-494 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkEchoWc",
    registry: "node",
    order: 21,
    optional: None,
    kind: CheckKind::Node(check_echo_wc),
};

fn check_echo_wc(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, cmds) = p.ast.get(t) else { return };
    let &[a, b] = &cmds[..] else { return };
    let acmd = oversimplify(p.ast, a);
    let bcmd = oversimplify(p.ast, b);
    let count_msg = |c: &mut Comments| c.style(t, 2000, "See if you can use ${#variable} instead.");
    if acmd == ["echo", "${VAR}"] {
        match &bcmd[..] {
            [x, y] if x == "wc" && y == "-c" => count_msg(c),
            [x, y] if x == "wc" && y == "-m" => count_msg(c),
            _ => {}
        }
    }
}
