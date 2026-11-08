// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArrayValueUsedAsIndex`, lines 4947-5021 (e11f964)

use std::collections::HashMap;

use shelllint_analyzer::analyzerlib::{DataSource, DataType, Params, make_comment};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    e4m, get_braced_modifier, get_braced_reference, get_path, get_word_parts, oversimplify,
};
use shelllint_core::interface::{Severity, TokenComment};

use crate::support::do_variable_flow_analysis;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArrayValueUsedAsIndex",
    registry: "tree",
    order: 13,
    optional: None,
    kind: CheckKind::Tree(check_array_value_used_as_index),
};

type VarMap = HashMap<String, (Id, Vec<(Id, String)>)>;

fn check_array_value_used_as_index(p: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let ast = p.ast;

    let write = |state: &mut VarMap, base: Id, _t: Id, name: &str, data: &DataType| {
        if let Token::T_ForIn(..) = ast.get(base) {
            if let DataType::DataString(DataSource::SourceFrom(words)) = data {
                let arrays: Vec<(Id, String)> = words
                    .iter()
                    .filter_map(|&x| get_array_name(p, x).map(|n| (x, n)))
                    .collect();
                state.insert(name.to_string(), (base, arrays));
                return vec![];
            }
        }
        state.remove(name);
        vec![]
    };

    let read = |state: &mut VarMap, _base: Id, t: Id, name: &str| -> Vec<TokenComment> {
        let Some((loop_id, arrays)) = state.get(name) else {
            return vec![];
        };
        let Some((array_ref, array_name)) = get_array_if_used_as_index(p, name, t) else {
            return vec![];
        };
        // Is this one of the 'for' arrays?
        let Some((loop_word, _)) = arrays.iter().find(|(_, n)| *n == array_name) else {
            return vec![];
        };
        // Are we still in this loop?
        if !get_path(ast, t).contains(loop_id) {
            return vec![];
        }
        vec![
            make_comment(
                Severity::WarningC,
                *loop_word,
                2302,
                "This loops over values. To loop over keys, use \"${!array[@]}\".",
            ),
            make_comment(
                Severity::WarningC,
                array_ref,
                2303,
                format!(
                    "{} is an array value, not a key. Use directly or loop over keys instead.",
                    e4m(name)
                ),
            ),
        ]
    };

    do_variable_flow_analysis(read, write, HashMap::new(), &p.variable_flow)
}

fn get_array_name(p: &Params<'_>, t: Id) -> Option<String> {
    let parts = get_word_parts(p.ast, t);
    let [part] = parts[..] else {
        return None;
    };
    let Token::T_DollarBraced(_, l) = p.ast.get(part) else {
        return None;
    };
    let str = oversimplify(p.ast, *l).concat();
    if get_braced_modifier(&str) == "[@]" && !str.starts_with('!') {
        Some(get_braced_reference(&str))
    } else {
        None
    }
}

// This is much uglier than it should be
fn get_array_if_used_as_index(p: &Params<'_>, name: &str, t: Id) -> Option<(Id, String)> {
    let ast = p.ast;
    match ast.get(t) {
        Token::T_DollarBraced(_, l) => {
            let r = get_braced_reference(&oversimplify(ast, *l).concat());
            if r != name {
                return None;
            }
            // We found a $name. Look up the chain to see if it's ${arr[$name]}
            let list = ast.parent(t)?;
            if !matches!(ast.get(list), Token::T_NormalWord(_)) {
                return None;
            }
            let pd = ast.parent(list)?;
            let Token::T_DollarBraced(_, parent_list) = ast.get(pd) else {
                return None;
            };
            let parts = get_word_parts(ast, *parent_list);
            if parts.len() < 3
                || !matches!(ast.get(parts[0]), Token::T_Literal(_))
                || !matches!(ast.get(parts[2]), Token::T_Literal(_))
            {
                return None;
            }
            let index = parts[1];
            let str = oversimplify(ast, list).concat();
            let modifier = get_braced_modifier(&str);
            if index != t {
                return None;
            }
            if !modifier.starts_with("[${VAR}]") {
                return None;
            }
            Some((t, get_braced_reference(&str)))
        }

        Token::T_NormalWord(_) => {
            // We found just name. Check if it's part of ${something[name]}
            let parent = ast.parent(t)?;
            if !matches!(ast.get(parent), Token::T_DollarBraced(..)) {
                return None;
            }
            let str = oversimplify(ast, t).concat();
            let modifier = get_braced_modifier(&str);
            if !modifier.starts_with(&format!("[{}]", name)) {
                return None;
            }
            Some((parent, get_braced_reference(&str)))
        }

        Token::TA_Variable(r, idx) if idx.is_empty() => {
            // We found arithmetic name. See if it's part of arithmetic arr[name]
            if r != name {
                return None;
            }
            let seq_id = ast.parent(t)?;
            match ast.get(seq_id) {
                Token::TA_Sequence(el) if el.len() == 1 && el[0] == t => {}
                _ => return None,
            }
            let parent = ast.parent(seq_id)?;
            match ast.get(parent) {
                Token::TA_Variable(array_name, el) if el.len() == 1 && el[0] == seq_id => {
                    Some((parent, array_name.clone()))
                }
                _ => None,
            }
        }

        _ => None,
    }
}
