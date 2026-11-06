// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkSshCommandString`, lines 708-721 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{arguments, is_constant, oversimplify};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkSshCommandString",
    registry: "command",
    order: 14,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("ssh"), check_ssh_command_string),
};

fn check_ssh_command_string(p: &Params<'_>, t: Id, c: &mut Comments) {
    f(p, &arguments(p.ast, t), c);
}

fn is_option(p: &Params<'_>, x: Id) -> bool {
    oversimplify(p.ast, x).concat().starts_with('-')
}

fn f(p: &Params<'_>, args: &[Id], c: &mut Comments) {
    let (opts, rest): (Vec<Id>, Vec<Id>) = args.iter().partition(|&&x| is_option(p, x));
    // ([], hostport:r@(_:_))
    if opts.is_empty() && rest.len() >= 2 {
        check_arg(p, *rest.last().unwrap(), c);
    }
}

fn check_arg(p: &Params<'_>, arg: Id, c: &mut Comments) {
    if let Token::T_NormalWord(l) = p.ast.get(arg) {
        if let [q] = l.as_slice() {
            if let Token::T_DoubleQuoted(parts) = p.ast.get(*q) {
                if let Some(&x) = parts.iter().find(|&&x| !is_constant(p.ast, x)) {
                    c.info(x, 2029, "Note that, unescaped, this expands on the client side.");
                }
            }
        }
    }
}
