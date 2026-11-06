// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkFunctionDeclarations`, lines 2975-2995 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{FunctionKeyword, FunctionParentheses, Id, Token};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkFunctionDeclarations",
    registry: "node",
    order: 54,
    optional: None,
    kind: CheckKind::Node(check_function_declarations),
};

fn check_function_declarations(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Function(FunctionKeyword(has_keyword), FunctionParentheses(has_parens), _, _) =
        p.ast.get(t)
    else {
        return;
    };
    let (has_keyword, has_parens) = (*has_keyword, *has_parens);
    let for_sh = |c: &mut Comments| {
        if has_keyword && has_parens {
            c.warn(t, 2112, "'function' keyword is non-standard. Delete it.");
        }
        if has_keyword && !has_parens {
            c.warn(
                t,
                2113,
                "'function' keyword is non-standard. Use 'foo()' instead of 'function foo'.",
            );
        }
    };
    match p.shell_type {
        Shell::Bash => {}
        Shell::Ksh => {
            if has_keyword && has_parens {
                c.err(t, 2111, "ksh does not allow 'function' keyword and '()' at the same time.");
            }
        }
        Shell::Dash | Shell::BusyboxSh | Shell::Sh => for_sh(c),
    }
}
