// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkAliasUsedInSameParsingUnit`, lines 4406-4449 (e11f964)

use std::collections::HashMap;

use shellcheck_analyzer::analyzerlib::{Comments, Params, should_ignore_code};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    get_command_sequences, get_literal_string_def, get_unquoted_literal, is_variable_name,
};
use shellcheck_core::interface::TokenComment;

use crate::support::is_sourced;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkAliasUsedInSameParsingUnit",
    registry: "tree",
    order: 12,
    optional: None,
    kind: CheckKind::Tree(check_alias_used_in_same_parsing_unit),
};

fn check_alias_used_in_same_parsing_unit(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    // Get all root commands
    let commands: Vec<Id> = get_command_sequences(params.ast, root).into_iter().flatten().collect();
    // Group them by whether they start on the same line where the previous one ended
    let units = group_by_link(|a, b| follows_on_line(params, a, b), &commands);
    let mut c = Comments::new();
    for unit in &units {
        check_unit(params, unit, &mut c);
    }
    c.into_vec()
}

fn line_span(params: &Params<'_>, t: Id) -> (u32, u32) {
    let (start, end) = params.ast.span(t);
    (start.line, end.line)
}

fn follows_on_line(params: &Params<'_>, a: Id, b: Id) -> bool {
    let (_, end) = line_span(params, a);
    let (start, _) = line_span(params, b);
    end == start
}

fn check_unit(params: &Params<'_>, unit: &[Id], c: &mut Comments) {
    let mut state: HashMap<String, Id> = HashMap::new();
    for &t in unit {
        params.ast.analyze(t, &mut |x| find_commands(params, x, &mut state, c), &mut |_| {});
    }
}

fn find_commands(params: &Params<'_>, t: Id, state: &mut HashMap<String, Id>, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = params.ast.get(t) else { return };
    let Some((&cmd, args)) = words.split_first() else { return };
    match get_unquoted_literal(params.ast, cmd) {
        Some(name) if name == "alias" => {
            for &arg in args {
                add_alias(params, arg, state);
            }
        }
        Some(name) if !name.contains('/') => {
            if let Some(&alias) = state.get(&name) {
                if !(is_sourced(params, t) || should_ignore_code(params, 2262, alias)) {
                    c.warn(
                        alias,
                        2262,
                        "This alias can't be defined and used in the same parsing unit. Use a function instead.",
                    );
                    c.info(
                        t,
                        2263,
                        "Since they're in the same parsing unit, this command will not refer to the previously mentioned alias.",
                    );
                }
            }
        }
        _ => {}
    }
}

fn add_alias(params: &Params<'_>, arg: Id, state: &mut HashMap<String, Id>) {
    let s = get_literal_string_def(params.ast, "-", arg);
    let (name, value) = match s.find('=') {
        Some(i) => s.split_at(i),
        None => (s.as_str(), ""),
    };
    if is_variable_name(name) && !value.is_empty() {
        // insertWith (\new old -> old): keep the first instance
        state.entry(name.to_string()).or_insert(arg);
    }
}

fn group_by_link<A: Copy>(f: impl Fn(A, A) -> bool, list: &[A]) -> Vec<Vec<A>> {
    let mut out: Vec<Vec<A>> = Vec::new();
    let Some((&first, rest)) = list.split_first() else { return out };
    let mut current = first;
    let mut span = vec![first];
    for &next in rest {
        if f(current, next) {
            span.push(next);
        } else {
            out.push(std::mem::take(&mut span));
            span.push(next);
        }
        current = next;
    }
    out.push(span);
    out
}
