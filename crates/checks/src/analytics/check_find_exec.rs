// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkFindExec`, lines 793-828 (e11f964)

use shellcheck_analyzer::analyzerlib::{is_command, Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::get_literal_string;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkFindExec",
    registry: "node",
    order: 28,
    optional: None,
    kind: CheckKind::Node(check_find_exec),
};

fn check_find_exec(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let Some((_, r)) = words.split_first() else { return };
    if !is_command(p.ast, t, "find") {
        return;
    }
    let broke = broken(p.ast, r, false, c);
    if broke {
        let word_id = *words.last().unwrap();
        c.err(
            word_id,
            2067,
            "Missing ';' or + terminating -exec. You can't use |/||/&&, and ';' has to be a separate, quoted argument.",
        );
    }
}

fn broken(ast: &Ast, ws: &[Id], mut v: bool, c: &mut Comments) -> bool {
    for &w in ws {
        if v {
            for x in from_word(ast, w) {
                warn_for(ast, x, c);
            }
        }
        match get_literal_string(ast, w).as_deref() {
            Some("-exec") => v = true,
            Some("-execdir") => v = true, // n.b. -execdir ;/+ is not defined in POSIX
            Some("-ok") => v = true,      // n.b. -ok + is not defined in POSIX
            Some("-okdir") => v = true,   // n.b. -okdir ;/+ is not defined in POSIX
            Some("+") => v = false,
            Some(";") => v = false,
            _ => {}
        }
    }
    v
}

fn should_warn(ast: &Ast, x: Id) -> bool {
    matches!(
        ast.get(x),
        Token::T_DollarExpansion(_) | Token::T_Backticked(_) | Token::T_Glob(_) | Token::T_Extglob(..)
    )
}

fn warn_for(ast: &Ast, x: Id, c: &mut Comments) {
    if should_warn(ast, x) {
        c.info(x, 2014, "This will expand once before find runs, not per file found.");
    }
}

fn from_word(ast: &Ast, w: Id) -> Vec<Id> {
    match ast.get(w) {
        Token::T_NormalWord(l) => l.clone(),
        _ => vec![],
    }
}
