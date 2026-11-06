// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkMultiDimensionalArrays`, lines 649-662 (e11f964)

use std::sync::LazyLock;

use regex::Regex;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, oversimplify};
use shellcheck_core::interface::Shell;
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkMultiDimensionalArrays",
    registry: "shell-support",
    order: 4,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash], f),
};

fn f(p: &Params<'_>, token: Id, c: &mut Comments) {
    match p.ast.get(token) {
        Token::T_Assignment(_, _, indices, _) => {
            if indices.len() >= 2 {
                about(indices[1], c);
            }
        }
        Token::T_IndexedElement(indices, _) => {
            if indices.len() >= 2 {
                about(indices[1], c);
            }
        }
        Token::T_DollarBraced(_, l) => {
            if is_multi_dim(p, *l) {
                about(token, c);
            }
        }
        _ => {}
    }
}

fn about(t: Id, c: &mut Comments) {
    c.warn(t, 2180, "Bash does not support multidimensional arrays. Use 1D or associative arrays.");
}

// Fixme, this matches ${foo:- [][]} and such as well
static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^\\[.*\\]\\[.*\\]"));

fn is_multi_dim(p: &Params<'_>, l: Id) -> bool {
    matches(&get_braced_modifier(&oversimplify(p.ast, l).concat()), &RE)
}
