// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkExpr`, lines 284-337 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    arguments, get_command_token_or_this, get_literal_string, get_word_parts, is_glob,
    only_literal_string, will_split,
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkExpr",
    registry: "command",
    order: 2,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("expr"), check_expr),
};

fn check_expr(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    if words(p, &args).iter().all(|w| !EXCEPTIONS.contains(&w.as_str())) {
        c.style(
            get_command_token_or_this(p.ast, t),
            2003,
            "expr is antiquated. Consider rewriting this using $((..)), ${} or [[ ]].",
        );
    }

    if let [lhs, op, rhs] = args[..] {
        check_op(p, lhs, c);
        let parts = get_word_parts(p.ast, op);
        if let [part] = parts[..] {
            match p.ast.get(part) {
                Token::T_Glob(s) if s == "*" => {
                    c.err(
                        op,
                        2304,
                        "* must be escaped to multiply: \\*. Modern $((x * y)) avoids this issue.",
                    );
                }
                Token::T_Literal(s) if s == ":" && is_glob(p.ast, rhs) => {
                    c.warn(rhs, 2305, "Quote regex argument to expr to avoid it expanding as a glob.");
                }
                _ => {}
            }
        }
        return;
    }

    if let [single] = args[..] {
        if !will_split(p.ast, single) {
            c.warn(
                single,
                2307,
                "'expr' expects 3+ arguments but sees 1. Make sure each operator/operand is a separate argument, and escape <>&|.",
            );
            return;
        }
    }

    if let [first, second] = args[..] {
        if only_literal_string(p.ast, first) != "length"
            && !(will_split(p.ast, first) || will_split(p.ast, second))
        {
            check_op(p, first, c);
            c.warn(
                t,
                2307,
                "'expr' expects 3+ arguments, but sees 2. Make sure each operator/operand is a separate argument, and escape <>&|.",
            );
            return;
        }
    }

    if let Some((&first, rest)) = args.split_first() {
        check_op(p, first, c);
        for &r in rest {
            // We already find 95%+ of multiplication and regex earlier, so don't bother classifying this further.
            if is_glob(p.ast, r) {
                c.warn(r, 2306, "Escape glob characters in arguments to expr to avoid pathname expansion.");
            }
        }
    }
}

/// These operators are hard to replicate in POSIX
const EXCEPTIONS: [&str; 9] = [
    ":", "<", ">", "<=", ">=", // We can offer better suggestions for these
    "match", "length", "substr", "index",
];

/// `words = mapMaybe getLiteralString`
fn words(p: &Params<'_>, args: &[Id]) -> Vec<String> {
    args.iter().filter_map(|&a| get_literal_string(p.ast, a)).collect()
}

fn check_op(p: &Params<'_>, side: Id, c: &mut Comments) {
    let msg = match get_literal_string(p.ast, side).as_deref() {
        Some("match") => "'expr match' has unspecified results. Prefer 'expr str : regex'.",
        Some("length") => "'expr length' has unspecified results. Prefer ${#var}.",
        Some("substr") => "'expr substr' has unspecified results. Prefer 'cut' or ${var#???}.",
        Some("index") => {
            "'expr index' has unspecified results. Prefer x=${var%%[chars]*}; $((${#x}+1))."
        }
        _ => return,
    };
    c.info(side, 2308, msg);
}
