// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkGlobsAsOptions`, lines 2743-2760 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_command_basename, oversimplify};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkGlobsAsOptions",
    registry: "node",
    order: 42,
    optional: None,
    kind: CheckKind::Node(check_globs_as_options),
};

fn check_globs_as_options(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    let ast = p.ast;
    let Token::T_SimpleCommand(_, args) = ast.get(cmd) else { return };
    let basename = get_command_basename(ast, cmd).unwrap_or_default();
    if basename == "echo" || basename == "printf" || p.has_noglob {
        return;
    }

    let is_end_of_args = |t: Id| {
        matches!(oversimplify(ast, t).concat().as_str(), "--" | ":::" | "::::")
    };

    for &v in args.iter().skip(1).take_while(|&&a| !is_end_of_args(a)) {
        // check v@(T_NormalWord _ (T_Glob id s:_)) | s == "*" || s == "?"
        if let Token::T_NormalWord(parts) = ast.get(v) {
            if let Some(&first) = parts.first() {
                if let Token::T_Glob(s) = ast.get(first) {
                    if s == "*" || s == "?" {
                        c.info(first, 2035, "Use ./*glob* or -- *glob* so names with dashes won't become options.");
                    }
                }
            }
        }
    }
}
