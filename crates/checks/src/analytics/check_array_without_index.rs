// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArrayWithoutIndex`, lines 1028-1061 (e11f964)

use std::collections::HashSet;

use shellcheck_analyzer::analyzerlib::{DataType, Params, make_comment};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::astlib::get_literal_string;
use shellcheck_core::data::ARRAY_VARIABLES;
use shellcheck_core::interface::{Severity, Shell, TokenComment};

use crate::support::do_variable_flow_analysis;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArrayWithoutIndex",
    registry: "tree",
    order: 6,
    optional: None,
    kind: CheckKind::Tree(check_array_without_index),
};

fn check_array_without_index(p: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let default_set: HashSet<String> = if p.shell_type == Shell::Bash {
        ARRAY_VARIABLES.iter().map(|s| s.to_string()).collect()
    } else {
        HashSet::new()
    };

    let read_f = |s: &mut HashSet<String>, _base: Id, token: Id, _name: &str| -> Vec<TokenComment> {
        if let Token::T_DollarBraced(_, inner) = p.ast.get(token) {
            if let Some(name) = get_literal_string(p.ast, *inner) {
                if s.contains(&name) {
                    return vec![make_comment(
                        Severity::WarningC,
                        token,
                        2128,
                        "Expanding an array without an index only gives the first element.",
                    )];
                }
            }
        }
        vec![]
    };

    let write_f = |s: &mut HashSet<String>, _base: Id, expr: Id, name: &str, dt: &DataType| -> Vec<TokenComment> {
        if let (Token::T_Assignment(mode, aname, idx, _), DataType::DataString(_)) = (p.ast.get(expr), dt) {
            if idx.is_empty() {
                let is_array = s.contains(aname);
                return if !is_array {
                    vec![]
                } else {
                    match mode {
                        AssignmentMode::Assign => vec![make_comment(
                            Severity::WarningC,
                            expr,
                            2178,
                            "Variable was used as an array but is now assigned a string.",
                        )],
                        AssignmentMode::Append => vec![make_comment(
                            Severity::WarningC,
                            expr,
                            2179,
                            "Use array+=(\"item\") to append items to an array.",
                        )],
                    }
                };
            }
        }
        if let DataType::DataArray(_) = dt {
            s.insert(name.to_string());
            return vec![];
        }
        let is_indexed = matches!(p.ast.get(expr), Token::T_Assignment(_, _, idx, _) if !idx.is_empty());
        if is_indexed {
            s.insert(name.to_string());
        } else {
            s.remove(name);
        }
        vec![]
    };

    do_variable_flow_analysis(read_f, write_f, default_set, &p.variable_flow)
}
