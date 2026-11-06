// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUuoeVar`, lines 1793-1818 (e11f964)

use shellcheck_analyzer::analyzerlib::{
    Comments, Params, is_unqualified_command, token_is_just_command_output,
};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::only_literal_string;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUuoeVar",
    registry: "node",
    order: 25,
    optional: None,
    kind: CheckKind::Node(check_uuoe_var),
};

fn check_uuoe_var(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Backticked(l) | Token::T_DollarExpansion(l) => {
            if let &[cmd] = &l[..] {
                check(p, t, cmd, c);
            }
        }
        _ => {}
    }
}

fn could_be_optimized(ast: &Ast, f: Id) -> bool {
    match ast.get(f) {
        Token::T_Glob(_) | Token::T_Extglob(..) | Token::T_BraceExpansion(_) => false,
        Token::T_NormalWord(l) | Token::T_DoubleQuoted(l) => {
            l.iter().all(|&x| could_be_optimized(ast, x))
        }
        _ => true,
    }
}

fn check(p: &Params<'_>, id: Id, cmd: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, cmds) = p.ast.get(cmd) else { return };
    let &[r] = &cmds[..] else { return };
    let Token::T_Redirecting(_, inner) = p.ast.get(r) else { return };
    warn_for_echo(p, id, *inner, c);
}

fn is_covered(p: &Params<'_>, first: Id, rest: &[Id]) -> bool {
    rest.is_empty() && token_is_just_command_output(p.ast, first)
}

fn warn_for_echo(p: &Params<'_>, id: Id, t: Id, c: &mut Comments) {
    let Some(vars) = check_unqualified_command(p, "echo", t) else { return };
    let Some((&first, rest)) = vars.split_first() else { return };
    if !(is_covered(p, first, rest) || only_literal_string(p.ast, first).starts_with('-'))
        && vars.iter().all(|&v| could_be_optimized(p.ast, v))
    {
        c.style(
            id,
            2116,
            "Useless echo? Instead of 'cmd $(echo foo)', just use 'cmd foo'.",
        );
    }
}

// Not ported elsewhere: returns the arguments (after the command word) when `t`
// is a simple command invoking the unqualified command `name`.
fn check_unqualified_command(p: &Params<'_>, name: &str, t: Id) -> Option<Vec<Id>> {
    if let Token::T_SimpleCommand(_, words) = p.ast.get(t) {
        if let Some((_, rest)) = words.split_first() {
            if is_unqualified_command(p.ast, t, name) {
                return Some(rest.to_vec());
            }
        }
    }
    None
}
