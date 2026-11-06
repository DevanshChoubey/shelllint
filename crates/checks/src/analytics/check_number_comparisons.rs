// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkNumberComparisons`, lines 1268-1386 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params, StackData};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_analyzer::cfg_analysis::{NumericalStatus, get_incoming_state};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::{
    get_braced_reference, get_literal_string_def, get_word_parts, is_quotes, is_variable_name, only_literal_string,
    oversimplify,
};
use shellcheck_core::data::ARITHMETIC_BINARY_TEST_OPS;
use shellcheck_core::interface::Shell;
use shellcheck_core::regex::{match_regex, mk_regex};

use crate::support::has_floating_point;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkNumberComparisons",
    registry: "node",
    order: 8,
    optional: None,
    kind: CheckKind::Node(check_number_comparisons),
};

static FLOAT_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("^[-+]?[0-9]+\\.[0-9]+$"));

fn is_lt_gt(op: &str) -> bool {
    ["<", "\\<", ">", "\\>"].contains(&op)
}

fn is_le_ge(op: &str) -> bool {
    ["<=", "\\<=", ">=", "\\>="].contains(&op)
}

fn eqv(op: &str) -> String {
    if let Some(s) = op.strip_prefix('\\') {
        return eqv(s);
    }
    match op {
        "<" => "-lt",
        ">" => "-gt",
        "<=" => "-le",
        ">=" => "-ge",
        _ => "the numerical equivalent",
    }
    .to_string()
}

fn invert(op: &str) -> &'static str {
    if let Some(s) = op.strip_prefix('\\') {
        return invert(s);
    }
    match op {
        "<=" => ">",
        ">=" => "<",
        // Non-exhaustive in Haskell; only called when is_le_ge holds.
        _ => unreachable!("invert called on {op:?}"),
    }
}

fn check_number_comparisons(p: &Params<'_>, id: Id, c: &mut Comments) {
    let Token::TC_Binary(typ, op, lhs, rhs) = p.ast.get(id) else { return };
    let (typ, lhs, rhs) = (*typ, *lhs, *rhs);
    let op = op.as_str();
    let ast = p.ast;

    let has_string_comparison = p.shell_type != Shell::Sh;
    let esc = if typ == ConditionType::SingleBracket { "\\" } else { "" };

    let is_fraction = |t: Id| -> bool {
        match &oversimplify(ast, t)[..] {
            [v] => match_regex(&FLOAT_REGEX, v).is_some(),
            _ => false,
        }
    };

    let check_decimals = |c: &mut Comments, hs: Id| {
        if is_fraction(hs) && !has_floating_point(p) {
            c.err(
                hs,
                2072,
                "Decimals are not supported. Either use integers only, or use bc or awk to compare.",
            );
        }
    };

    let is_num = |t: Id| -> bool {
        let parts = get_word_parts(ast, t);
        match &parts[..] {
            [_] if matches!(ast.get(parts[0]), Token::T_DollarArithmetic(_)) => true,
            &[b] if matches!(ast.get(b), Token::T_DollarBraced(..)) => {
                let Token::T_DollarBraced(_, cc) = ast.get(b) else { return false };
                let s = oversimplify(ast, *cc).concat();
                let var = get_braced_reference(&s);
                (|| {
                    let cfga = p.cfg_analysis.as_deref()?;
                    let state = get_incoming_state(cfga, b)?;
                    let value = state.variable_in_scope(&var)?;
                    Some(value.variable_value.numerical_status >= NumericalStatus::NumericalStatusMaybe)
                })()
                .unwrap_or(false)
            }
            _ => match &oversimplify(ast, t)[..] {
                [v] => v.chars().all(|ch| ch.is_ascii_digit()),
                _ => false,
            },
        }
    };

    // `s `elem` assignedVariables`, without building the list.
    let is_assigned_variable = |s: &str| -> bool {
        p.variable_flow.iter().any(|t| matches!(t, StackData::Assignment(_, _, name, _) if name == s))
    };

    let is_non_num = |t: Id| -> bool {
        let num_char = |x: char| x.is_ascii_digit() || "+-. ".contains(x);
        !only_literal_string(ast, t).chars().all(num_char)
    };

    let seqv = |op: &str| -> String {
        match op {
            "-ge" => format!("! a {esc}< b"),
            "-gt" => format!("{esc}>"),
            "-le" => format!("! a {esc}> b"),
            "-lt" => format!("{esc}<"),
            "-eq" => "=".to_string(),
            "-ne" => "!=".to_string(),
            _ => "the string equivalent".to_string(),
        }
    };

    let check_string = |c: &mut Comments, t: Id| {
        let as_string = get_literal_string_def(ast, "\0", t);
        let is_var = is_variable_name(&as_string);
        let kind = if is_var { "a variable" } else { "an arithmetic expression" };
        let fix = if is_var { "$var" } else { "$((expr))" };
        if is_non_num(t) {
            if typ == ConditionType::SingleBracket {
                c.err(
                    t,
                    2170,
                    format!(
                        "Invalid number for {op}. Use {} to compare as string (or use {fix} to expand as {kind}).",
                        seqv(op)
                    ),
                );
            } else {
                // We should warn if any of the following holds:
                //   The string is not a variable name
                //   Any part of it is quoted
                //   It's not a recognized variable name
                if !is_var
                    || get_word_parts(ast, t).into_iter().any(|x| is_quotes(ast, x))
                    || !is_assigned_variable(&as_string)
                {
                    c.warn(
                        t,
                        2309,
                        format!(
                            "{op} treats this as {kind}. Use {} to compare as string (or expand explicitly with {fix}).",
                            seqv(op)
                        ),
                    );
                }
            }
        }
    };

    if is_num(lhs) || is_num(rhs) {
        if is_lt_gt(op) {
            c.err(id, 2071, format!("{op} is for string comparisons. Use {} instead.", eqv(op)));
        }
        if is_le_ge(op) && has_string_comparison {
            c.err(id, 2071, format!("{op} is not a valid operator. Use {} .", eqv(op)));
        }
    } else {
        if is_le_ge(op) || is_lt_gt(op) {
            for hs in [lhs, rhs] {
                check_decimals(c, hs);
            }
        }

        if is_le_ge(op) && has_string_comparison {
            c.err(
                id,
                2122,
                format!("{op} is not a valid operator. Use '! a {esc}{} b' instead.", invert(op)),
            );
        }

        if typ == ConditionType::SingleBracket && (op == "<" || op == ">") {
            match p.shell_type {
                // These are unsupported and will be caught by bashism checks.
                Shell::Sh => {}
                Shell::Dash | Shell::BusyboxSh => {
                    c.err(id, 2073, format!("Escape \\{op} to prevent it redirecting."));
                }
                _ => {
                    c.err(
                        id,
                        2073,
                        format!("Escape \\{op} to prevent it redirecting (or switch to [[ .. ]])."),
                    );
                }
            }
        }
    }

    if ARITHMETIC_BINARY_TEST_OPS.contains(&op) {
        for hs in [lhs, rhs] {
            check_decimals(c, hs);
        }
        for t in [lhs, rhs] {
            check_string(c, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn fires(script: &str) -> bool {
        !run_check("Analytics.hs", "checkNumberComparisons", "node", script).expect("check runs").is_empty()
    }

    macro_rules! t {
        ($name:ident, $fires:expr, $script:expr) => {
            #[test]
            fn $name() {
                assert_eq!(fires($script), $fires, "{}", $script);
            }
        };
    }

    t!(prop1, true, "[[ $foo < 3 ]]");
    t!(prop2, true, "[[ 0 >= $(cmd) ]]");
    t!(prop3, false, "[[ $foo ]] > 3");
    t!(prop4, true, "[[ $foo > 2.72 ]]");
    t!(prop5, true, "[[ $foo -le 2.72 ]]");
    t!(prop6, true, "[[ 3.14 -eq $foo ]]");
    t!(prop7, false, "[[ 3.14 == $foo ]]");
    t!(prop8, true, "[ foo <= bar ]");
    t!(prop9, true, "[ foo \\>= bar ]");
    t!(prop11, true, "[ $foo -eq 'N' ]");
    t!(prop12, true, "[ x$foo -gt x${N} ]");
    t!(prop13, true, "[ $foo > $bar ]");
    t!(prop14, false, "[[ foo < bar ]]");
    t!(prop15, false, "[ $foo '>' $bar ]");
    t!(prop16, true, "[ foo -eq 'y' ]");
    t!(prop17, true, "[[ 'foo' -eq 2 ]]");
    t!(prop18, true, "[[ foo -eq 2 ]]");
    t!(prop19, false, "foo=1; [[ foo -eq 2 ]]");
    t!(prop20, true, "[[ 2 -eq / ]]");
    t!(prop21, true, "[[ foo -eq foo ]]");
    t!(prop22, true, "x=10; [[ $x > $z ]]");
    t!(prop23, true, "x=0; if [[ -n $def ]]; then x=$def; fi; while [ $x > $z ]; do lol; done");
    t!(prop24, true, "x=$RANDOM; [ $x > $z ]");
    t!(prop25, true, "[[ $((n++)) > $x ]]");
}
