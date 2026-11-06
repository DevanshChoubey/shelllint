// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArrayAssignmentIndices`, lines 3646-3677 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_associative_arrays;
use shellcheck_core::interface::TokenComment;

use crate::support::{run_node_analysis, surround_with};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArrayAssignmentIndices",
    registry: "tree",
    order: 10,
    optional: None,
    kind: CheckKind::Tree(check_array_assignment_indices),
};

fn check_array_assignment_indices(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    let assocs = get_associative_arrays(params.ast, root);
    let check = |p: &Params<'_>, t: Id, c: &mut Comments| {
        if let Token::T_Assignment(_, name, idx, arr) = p.ast.get(t) {
            if idx.is_empty() {
                if let Token::T_Array(list) = p.ast.get(*arr) {
                    let is_assoc = assocs.contains(name);
                    for &e in list {
                        check_element(p, is_assoc, e, c);
                    }
                }
            }
        }
    };
    run_node_analysis(&check, params, root)
}

fn check_element(params: &Params<'_>, is_associative: bool, t: Id, c: &mut Comments) {
    match params.ast.get(t) {
        Token::T_IndexedElement(_, v) => {
            if let Token::T_Literal(s) = params.ast.get(*v) {
                if s.is_empty() {
                    c.warn(
                        *v,
                        2192,
                        "This array element has no value. Remove spaces after = or use \"\" for empty string.",
                    );
                }
            }
        }
        Token::T_NormalWord(parts) => {
            let mut literal_equals = Vec::new();
            for &part in parts {
                let Token::T_Literal(s) = params.ast.get(part) else { continue };
                let before = s.split('=').next().unwrap_or("");
                let has_after = s.len() > before.len();
                if before.chars().all(|ch| ch.is_ascii_digit()) && has_after {
                    literal_equals.push(part);
                }
            }
            if literal_equals.is_empty() && is_associative {
                c.warn(
                    t,
                    2190,
                    "Elements in associative arrays need index, e.g. array=( [index]=value ) .",
                );
            } else {
                for id in literal_equals {
                    c.warn_with_fix(
                        id,
                        2191,
                        "The = here is literal. To assign by index, use ( [index]=value ) with no spaces. To keep as literal, quote it.",
                        surround_with(id, params, "\""),
                    );
                }
            }
        }
        _ => {}
    }
}
