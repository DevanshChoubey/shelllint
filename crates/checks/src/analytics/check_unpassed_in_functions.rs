// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnpassedInFunctions`, lines 3025-3107 (e11f964)

use std::collections::BTreeMap;

use shellcheck_analyzer::analyzerlib::{
    Comments, Params, StackData, get_variable_flow, should_ignore_code,
};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{e4m, get_braced_modifier, get_literal_string, get_path, oversimplify};
use shellcheck_core::interface::TokenComment;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnpassedInFunctions",
    registry: "tree",
    order: 5,
    optional: None,
    kind: CheckKind::Tree(check_unpassed_in_functions),
};

fn check_unpassed_in_functions(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    let ast = params.ast;

    // functionMap: Map.fromList keeps the last entry for a duplicate key.
    let mut function_map: BTreeMap<String, Id> = BTreeMap::new();
    ast.analyze(
        root,
        &mut |t| {
            if let Some((name, t)) = find_function(params, t) {
                function_map.insert(name, t);
            }
        },
        &mut |_| {},
    );

    // referenceList: (name, argumentless, command)
    let mut reference_list: Vec<(String, bool, Id)> = Vec::new();
    ast.analyze(
        root,
        &mut |t| {
            if let Some(x) = check_command(params, &function_map, t) {
                reference_list.push(x);
            }
        },
        &mut |_| {},
    );

    // referenceGroups = Map.elems $ foldr updateWith Map.empty referenceList
    // Each group keeps the original list order.
    let mut groups: BTreeMap<String, Vec<(String, bool, Id)>> = BTreeMap::new();
    for x in reference_list {
        groups.entry(x.0.clone()).or_default().push(x);
    }

    let mut c = Comments::default();
    for group in groups.values() {
        warn_for_group(params, &function_map, group, &mut c);
    }
    c.into_vec()
}

fn find_function(params: &Params<'_>, t: Id) -> Option<(String, Id)> {
    let Token::T_Function(_, _, name, body) = params.ast.get(t) else { return None };
    let flow = get_variable_flow(params, *body);
    if flow.iter().any(|x| is_positional_reference(params, t, x))
        && !flow.iter().any(is_positional_assignment)
    {
        Some((name.clone(), t))
    } else {
        None
    }
}

fn is_positional_assignment(x: &StackData) -> bool {
    match x {
        StackData::Assignment(_, _, str, _) => is_positional(str),
        _ => false,
    }
}

fn is_positional_reference(params: &Params<'_>, function: Id, x: &StackData) -> bool {
    match x {
        StackData::Reference(_, t, str) => {
            is_positional(str) && is_direct_child_of(params, *t, function) && !has_default_value(params, *t)
        }
        _ => false,
    }
}

fn is_direct_child_of(params: &Params<'_>, child: Id, parent: Id) -> bool {
    let found = get_path(params.ast, child)
        .into_iter()
        .find(|&x| matches!(params.ast.get(x), Token::T_Function(..) | Token::T_Script(..)));
    // T_Script is for sourced files
    match found {
        Some(function) => parent == function,
        None => false,
    }
}

fn check_command(
    params: &Params<'_>,
    function_map: &BTreeMap<String, Id>,
    t: Id,
) -> Option<(String, bool, Id)> {
    let Token::T_SimpleCommand(_, words) = params.ast.get(t) else { return None };
    let (&cmd, args) = words.split_first()?;
    let str = get_literal_string(params.ast, cmd)?;
    if !function_map.contains_key(&str) {
        return None;
    }
    Some((str, args.is_empty(), t))
}

fn is_positional(str: &str) -> bool {
    str == "*"
        || str == "@"
        || str == "#"
        || (str.chars().all(|c| c.is_ascii_digit()) && str != "0" && !str.is_empty())
}

/// True if t is a variable that specifies a default value,
/// such as ${1-x} or ${1:-x}.
fn has_default_value(params: &Params<'_>, t: Id) -> bool {
    match params.ast.get(t) {
        Token::T_DollarBraced(true, l) => {
            let str = oversimplify(params.ast, *l).concat();
            is_default_value_modifier(&get_braced_modifier(&str))
        }
        _ => false,
    }
}

fn is_default_value_modifier(str: &str) -> bool {
    let handles_default = "-+?";
    let mut cs = str.chars();
    match (cs.next(), cs.next()) {
        (Some(':'), Some(c)) => handles_default.contains(c),
        (Some(c), _) => handles_default.contains(c),
        _ => false,
    }
}

fn warn_for_group(
    params: &Params<'_>,
    function_map: &BTreeMap<String, Id>,
    group: &[(String, bool, Id)],
    c: &mut Comments,
) {
    let (name, _, _) = &group[0];
    let func = function_map[name];
    let ignoring = should_ignore_code(params, 2120, func);
    // Allow ignoring SC2120 on the function to ignore all calls
    if group.iter().all(|(_, b, _)| *b) && !ignoring {
        for (name, _, thing) in group {
            c.info(
                *thing,
                2119,
                format!("Use {} \"$@\" if function's $1 should mean script's $1.", e4m(name)),
            );
        }
        c.warn(
            func,
            2120,
            format!("{} references arguments, but none are ever passed.", name),
        );
    }
}
