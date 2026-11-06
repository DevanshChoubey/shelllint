// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkBlatantRecursion`, lines 4481-4509 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_command_name_and_token, get_command_sequences};

use crate::support::{fix_with, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkBlatantRecursion",
    registry: "node",
    order: 86,
    optional: None,
    kind: CheckKind::Node(check_blatant_recursion),
};

fn check_blatant_recursion(params: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Function(_, _, name, body) = params.ast.get(t) {
        let seqs = get_command_sequences(params.ast, *body);
        if let [seq] = &seqs[..] {
            if let Some(&first) = seq.first() {
                check_list(params, name, first, c);
            }
        }
    }
}

fn check_list(params: &Params<'_>, name: &str, t: Id, c: &mut Comments) {
    match params.ast.get(t) {
        Token::T_Backgrounded(t) => check_list(params, name, *t, c),
        Token::T_AndIf(lhs, _) => check_list(params, name, *lhs, c),
        Token::T_OrIf(lhs, _) => check_list(params, name, *lhs, c),
        Token::T_Pipeline(_, cmds) => {
            for &cmd in cmds {
                check_command(params, name, cmd, c);
            }
        }
        _ => {}
    }
}

fn check_command(params: &Params<'_>, name: &str, cmd: Id, c: &mut Comments) {
    let (invoked_m, t) = get_command_name_and_token(params.ast, true, cmd);
    let Some(invoked) = invoked_m else { return };
    if name != invoked {
        return;
    }
    c.err_with_fix(
        t,
        2264,
        "This function unconditionally re-invokes itself. Missing 'command'?",
        fix_with(vec![replace_start(t, params, 0, "command ")]),
    );
}
