// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSubshellAsTest`, lines 3737-3757 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;
use shellcheck_core::data::{BINARY_TEST_OPS, UNARY_TEST_OPS};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSubshellAsTest",
    registry: "node",
    order: 72,
    optional: None,
    kind: CheckKind::Node(check_subshell_as_test),
};

fn check_subshell_as_test(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Subshell(body) = p.ast.get(t) {
        if let &[w] = &body[..] {
            check(p, t, w, c);
        }
    }
}

fn check(p: &Params<'_>, id: Id, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Banged(w) => check(p, id, *w, c),
        Token::T_AndIf(w, _) => check(p, id, *w, c),
        Token::T_OrIf(w, _) => check(p, id, *w, c),
        Token::T_Pipeline(_, cmds) => {
            let &[r] = &cmds[..] else { return };
            let Token::T_Redirecting(_, cmd) = p.ast.get(r) else { return };
            let Token::T_SimpleCommand(assigns, words) = p.ast.get(*cmd) else { return };
            if !assigns.is_empty() {
                return;
            }
            if let [first, second, ..] = &words[..] {
                check_params(p, id, *first, *second, c);
            }
        }
        _ => {}
    }
}

fn check_params(p: &Params<'_>, id: Id, first: Id, second: Id, c: &mut Comments) {
    if get_literal_string(p.ast, first).is_some_and(|s| UNARY_TEST_OPS.contains(&s.as_str())) {
        c.err(id, 2204, "(..) is a subshell. Did you mean [ .. ], a test expression?");
    }
    if get_literal_string(p.ast, second).is_some_and(|s| BINARY_TEST_OPS.contains(&s.as_str())) {
        c.warn(id, 2205, "(..) is a subshell. Did you mean [ .. ], a test expression?");
    }
}
