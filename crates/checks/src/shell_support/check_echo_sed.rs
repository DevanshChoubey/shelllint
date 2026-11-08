// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkEchoSed`, lines 579-614 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::oversimplify;
use shelllint_core::interface::Shell;
use shelllint_core::regex::{match_regex, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkEchoSed",
    registry: "shell-support",
    order: 2,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash, Shell::Ksh], f),
};

// This should have used backreferences, but TDFA doesn't support them
static SED_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^s(.)([^\n]*)g?$"));

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Redirecting(lefts, r) => {
            let redirect_here_string = |&l: &Id| match p.ast.get(l) {
                Token::T_FdRedirect(_, target) => matches!(p.ast.get(*target), Token::T_HereString(_)),
                _ => false,
            };
            if lefts.iter().any(redirect_here_string) {
                let rcmd = oversimplify(p.ast, *r);
                check_sed(t, &rcmd, c);
            }
        }
        Token::T_Pipeline(_, cmds) => {
            if let &[a, b] = &cmds[..] {
                let acmd = oversimplify(p.ast, a);
                let bcmd = oversimplify(p.ast, b);
                if acmd == ["echo", "${VAR}"] {
                    check_sed(t, &bcmd, c);
                }
            }
        }
        _ => {}
    }
}

fn check_sed(id: Id, cmd: &[String], c: &mut Comments) {
    match cmd {
        [a, v] if a == "sed" => check_in(id, v, c),
        [a, e, v] if a == "sed" && e == "-e" => check_in(id, v, c),
        _ => {}
    }
}

fn is_simple_sed(s: &str) -> bool {
    let Some(g) = match_regex(&SED_RE, s) else { return false };
    let [h, rest] = &g[..] else { return false };
    let Some(h) = h.chars().next() else { return false };
    let delimiters = rest.chars().filter(|&ch| ch == h).count();
    delimiters == 2
}

fn check_in(id: Id, s: &str, c: &mut Comments) {
    if is_simple_sed(s) {
        c.style(id, 2001, "See if you can use ${variable//search/replace} instead.");
    }
}
