// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTestArgumentSplitting`, lines 3273-3351 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::{get_word_parts, is_array_expansion, is_brace_expansion, is_glob};
use shellcheck_core::data::ARITHMETIC_BINARY_TEST_OPS;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTestArgumentSplitting",
    registry: "node",
    order: 62,
    optional: None,
    kind: CheckKind::Node(check_test_argument_splitting),
};

fn check_test_argument_splitting(p: &Params<'_>, t: Id, c: &mut Comments) {
    use ConditionType::*;
    match p.ast.get(t) {
        Token::TC_Unary(typ, op, token) if is_glob(p.ast, *token) => {
            let (typ, token) = (*typ, *token);
            if op == "-v" {
                if typ == SingleBracket {
                    c.err(token, 2208, "Use [[ ]] or quote arguments to -v to avoid glob expansion.");
                }
            } else if typ == SingleBracket && p.shell_type == Shell::Ksh {
                // Ksh appears to stop processing after unrecognized tokens, so operators
                // will effectively work with globs, but only the first match.
                let mut chars = op.chars();
                if chars.next() == Some('-')
                    && matches!(chars.next(), Some(ch) if "bcdfgkprsuwxLhNOGRS".contains(ch))
                    && chars.next().is_none()
                {
                    c.warn(
                        token,
                        2245,
                        format!("{} only applies to the first expansion of this glob. Use a loop to check any/all.", op),
                    );
                }
            } else {
                c.err(token, 2144, format!("{} doesn't work with globs. Use a for loop.", op));
            }
        }
        Token::TC_Nullary(typ, token) => {
            let (typ, token) = (*typ, *token);
            check_braces(p, c, typ, token);
            check_globs(p, c, typ, token);
            if typ == DoubleBracket {
                check_arrays(p, c, typ, token);
            }
        }
        Token::TC_Unary(typ, _op, token) => check_all(p, c, *typ, *token),
        Token::TC_Binary(typ, op, lhs, rhs) if ARITHMETIC_BINARY_TEST_OPS.contains(&op.as_str()) => {
            let (typ, lhs, rhs) = (*typ, *lhs, *rhs);
            if typ == DoubleBracket {
                for tok in [lhs, rhs] {
                    check_arrays(p, c, typ, tok);
                    check_braces(p, c, typ, tok);
                }
            } else {
                for tok in [lhs, rhs] {
                    check_numerical_glob(p, c, typ, tok);
                    check_arrays(p, c, typ, tok);
                    check_braces(p, c, typ, tok);
                }
            }
        }
        Token::TC_Binary(typ, op, lhs, rhs) => {
            let (typ, lhs, rhs) = (*typ, *lhs, *rhs);
            if ["=", "==", "!=", "=~"].contains(&op.as_str()) {
                check_all(p, c, typ, lhs);
                check_arrays(p, c, typ, rhs);
                check_braces(p, c, typ, rhs);
            } else {
                for tok in [lhs, rhs] {
                    check_all(p, c, typ, tok);
                }
            }
        }
        _ => {}
    }
}

fn check_all(p: &Params<'_>, c: &mut Comments, typ: ConditionType, token: Id) {
    check_arrays(p, c, typ, token);
    check_braces(p, c, typ, token);
    check_globs(p, c, typ, token);
}

fn check_arrays(p: &Params<'_>, c: &mut Comments, typ: ConditionType, token: Id) {
    if get_word_parts(p.ast, token).into_iter().any(|x| is_array_expansion(p.ast, x)) {
        if typ == ConditionType::SingleBracket {
            c.warn(token, 2198, "Arrays don't work as operands in [ ]. Use a loop (or concatenate with * instead of @).");
        } else {
            c.err(token, 2199, "Arrays implicitly concatenate in [[ ]]. Use a loop (or explicit * instead of @).");
        }
    }
}

fn check_braces(p: &Params<'_>, c: &mut Comments, typ: ConditionType, token: Id) {
    if get_word_parts(p.ast, token).into_iter().any(|x| is_brace_expansion(p.ast, x)) {
        if typ == ConditionType::SingleBracket {
            c.warn(token, 2200, "Brace expansions don't work as operands in [ ]. Use a loop.");
        } else {
            c.err(token, 2201, "Brace expansion doesn't happen in [[ ]]. Use a loop.");
        }
    }
}

fn check_globs(p: &Params<'_>, c: &mut Comments, typ: ConditionType, token: Id) {
    if is_glob(p.ast, token) {
        if typ == ConditionType::SingleBracket {
            c.warn(token, 2202, "Globs don't work as operands in [ ]. Use a loop.");
        } else {
            c.err(token, 2203, "Globs are ignored in [[ ]] except right of =/!=. Use a loop.");
        }
    }
}

// Haskell is partial here: only SingleBracket is matched, and it is only called for it.
fn check_numerical_glob(p: &Params<'_>, c: &mut Comments, typ: ConditionType, token: Id) {
    if typ == ConditionType::SingleBracket {
        // var[x] and x*2 look like globs
        if p.shell_type != Shell::Ksh && is_glob(p.ast, token) {
            c.err(token, 2255, "[ ] does not apply arithmetic evaluation. Evaluate with $((..)) for numbers, or use string comparator for strings.");
        }
    }
}
