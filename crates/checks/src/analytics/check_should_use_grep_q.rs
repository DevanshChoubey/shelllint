// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkShouldUseGrepQ`, lines 3222-3251 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_command_basename;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkShouldUseGrepQ",
    registry: "node",
    order: 61,
    optional: None,
    kind: CheckKind::Node(check_should_use_grep_q),
};

fn check_should_use_grep_q(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::TC_Nullary(_, token) => check(p, c, t, true, *token),
        Token::TC_Unary(_, op, token) if op == "-n" => check(p, c, t, true, *token),
        Token::TC_Unary(_, op, token) if op == "-z" => check(p, c, t, false, *token),
        _ => {}
    }
}

fn check(p: &Params<'_>, c: &mut Comments, id: Id, bool_: bool, token: Id) {
    let Some(name) = get_final_grep(p, token) else { return };
    let op = if bool_ { "-n" } else { "-z" };
    let flip = if bool_ { "" } else { "! " };
    c.style(
        id,
        2143,
        format!(
            "Use {}{} -q instead of comparing output with [ {} .. ].",
            flip, name, op
        ),
    );
}

fn get_final_grep(p: &Params<'_>, t: Id) -> Option<String> {
    let cmds = get_pipeline(p, t)?;
    let last = *cmds.last()?;
    let name = get_command_basename(p.ast, last)?;
    if is_grep(&name) { Some(name) } else { None }
}

fn get_pipeline(p: &Params<'_>, t: Id) -> Option<Vec<Id>> {
    match p.ast.get(t) {
        Token::T_NormalWord(l) | Token::T_DoubleQuoted(l) | Token::T_DollarExpansion(l) => {
            if let &[x] = &l[..] { get_pipeline(p, x) } else { None }
        }
        Token::T_Pipeline(_, cmds) => Some(cmds.clone()),
        _ => None,
    }
}

fn is_grep(s: &str) -> bool {
    [
        "grep", "egrep", "fgrep", "bz3grep", "bzgrep", "xzgrep", "zgrep", "zipgrep", "zstdgrep",
    ]
    .contains(&s)
}
