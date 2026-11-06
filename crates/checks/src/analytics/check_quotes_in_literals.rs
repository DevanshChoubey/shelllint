// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkQuotesInLiterals`, lines 2374-2429 (e11f964)

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{
    DataSource, DataType, Params, is_param_to, is_quote_free, make_comment, supports_arrays,
};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;
use shellcheck_core::interface::{Severity, TokenComment};
use shellcheck_core::regex::{matches, mk_regex};

use crate::support::do_variable_flow_analysis;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkQuotesInLiterals",
    registry: "tree",
    order: 1,
    optional: None,
    kind: CheckKind::Tree(check_quotes_in_literals),
};

static QUOTE_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("\"|([/= ]|^)'|'( |$)|\\\\ "));

fn check_quotes_in_literals(params: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let ast = params.ast;
    let contains_quotes = |s: &str| matches(s, &QUOTE_REGEX);

    fn for_token(
        params: &Params<'_>,
        map: &HashMap<String, Id>,
        t: Id,
        contains_quotes: &dyn Fn(&str) -> bool,
    ) -> Option<Id> {
        let ast = params.ast;
        match ast.get(t) {
            Token::T_DollarBraced(_, inner) => {
                // skip getBracedReference here to avoid false positives on PE
                map.get(&oversimplify(ast, *inner).concat()).copied()
            }
            Token::T_DoubleQuoted(tokens) | Token::T_NormalWord(tokens) => {
                tokens.iter().find_map(|&x| for_token(params, map, x, contains_quotes))
            }
            _ => {
                if contains_quotes(&oversimplify(ast, t).concat()) {
                    Some(t)
                } else {
                    None
                }
            }
        }
    }

    let squashes_quotes = |t: Id| match ast.get(t) {
        Token::T_DollarBraced(_, l) => oversimplify(ast, *l).concat().starts_with('#'),
        _ => false,
    };

    let suggestion =
        if supports_arrays(params.shell_type) { "Use an array." } else { "Rewrite using set/\"$@\" or functions." };

    let write_f = |state: &mut HashMap<String, Id>, _: Id, _: Id, name: &str, d: &DataType| -> Vec<TokenComment> {
        if let DataType::DataString(DataSource::SourceFrom(values)) = d {
            let quoted_vars = values.iter().find_map(|&v| for_token(params, state, v, &contains_quotes));
            match quoted_vars {
                None => {
                    state.remove(name);
                }
                Some(x) => {
                    state.insert(name.to_string(), x);
                }
            }
        }
        vec![]
    };

    let read_f = |state: &mut HashMap<String, Id>, _: Id, expr: Id, name: &str| -> Vec<TokenComment> {
        match state.get(name) {
            Some(&j)
                if !is_param_to(ast, "eval", expr)
                    && !is_quote_free(ast, params.shell_type, expr)
                    && !squashes_quotes(expr) =>
            {
                vec![
                    make_comment(
                        Severity::WarningC,
                        j,
                        2089,
                        format!("Quotes/backslashes will be treated literally. {}", suggestion),
                    ),
                    make_comment(
                        Severity::WarningC,
                        expr,
                        2090,
                        "Quotes/backslashes in this variable will not be respected.",
                    ),
                ]
            }
            _ => vec![],
        }
    };

    do_variable_flow_analysis(read_f, write_f, HashMap::new(), &params.variable_flow)
}
