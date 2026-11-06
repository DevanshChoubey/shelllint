// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnassignedReferences`, lines 2649-2649 (e11f964)

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Params, StackData, make_comment};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    get_braced_modifier, get_braced_reference, get_path, is_variable_char, is_variable_name, oversimplify,
};
use shellcheck_core::data::{COMMON_COMMANDS, INTERNAL_VARIABLES};
use shellcheck_core::interface::{Severity, TokenComment};
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnassignedReferences",
    registry: "tree",
    order: 8,
    optional: None,
    kind: CheckKind::Tree(check_unassigned_references),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_TRUE: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnassignedReferences' True",
    registry: "optional-tree",
    order: 5,
    optional: Some(StaticCheckDescription {
        name: "check-unassigned-uppercase",
        id: &[2154],
        description: "Warn when uppercase variables are unassigned",
        positive: "echo $VAR",
        negative: "VAR=hello; echo $VAR",
    }),
    kind: CheckKind::Tree(check_unassigned_references_true),
};

fn check_unassigned_references(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    check_unassigned_references_(false, p, t)
}

fn check_unassigned_references_true(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    check_unassigned_references_(true, p, t)
}

//  :? or :- with optional array index and colon
static GUARD_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("^(\\[.*\\])?:?[-?]"));

// DIVERGENCE: `dist` is a private Levenshtein distance (the Haskell is the
// wiki's edit-distance implementation, which computes the same value).
fn dist(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ac) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, bc) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ac != bc);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn to_lower(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

fn match_(var: &str, candidate: &str) -> usize {
    if var != candidate && to_lower(var) == to_lower(candidate) {
        1
    } else {
        dist(&var.chars().collect::<Vec<_>>(), &candidate.chars().collect::<Vec<_>>())
    }
}

fn check_unassigned_references_(include_globals: bool, p: &Params<'_>, _t: Id) -> Vec<TokenComment> {
    let ast = p.ast;
    // (readMap, writeMap)
    let mut read_map: BTreeMap<String, Id> = BTreeMap::new();
    let mut write_map: BTreeMap<String, ()> = BTreeMap::new();
    for s in &p.variable_flow {
        match s {
            StackData::Assignment(_, _, name, _) => {
                write_map.insert(name.clone(), ());
            }
            StackData::Reference(_, place, name) => {
                // insertWith (const id): keep the existing value
                read_map.entry(name.clone()).or_insert(*place);
            }
            _ => {}
        }
    }
    let default_assigned: Vec<&str> = INTERNAL_VARIABLES.iter().copied().filter(|s| !s.is_empty()).collect();

    let unassigned: Vec<(&String, Id)> = read_map
        .iter()
        .filter(|(k, _)| !write_map.contains_key(*k) && !default_assigned.contains(&k.as_str()))
        .map(|(k, v)| (k, *v))
        .collect();
    let written_vars: Vec<&String> = write_map.keys().filter(|k| is_variable_name(k)).collect();

    let get_best_match = |var: &str| -> Option<String> {
        let mut best: Vec<(&String, usize)> = written_vars.iter().map(|x| (*x, match_(var, x))).collect();
        best.sort_by_key(|(_, s)| *s);
        let (m, score) = best.first()?;
        let l = m.chars().count();
        let good = l > 3 && *score <= 1 || l > 7 && *score <= 2;
        if good { Some((*m).clone()) } else { None }
    };

    let is_local = |s: &str| s.chars().any(|c| c.is_lowercase());

    let warning_for_globals = |var: &str, place: Id| -> Option<TokenComment> {
        let m = get_best_match(var)?;
        Some(make_comment(
            Severity::InfoC,
            place,
            2153,
            format!("Possible misspelling: {} may not be assigned. Did you mean {}?", var, m),
        ))
    };

    let warning_for_locals = |var: &str, place: Id| -> Option<TokenComment> {
        let optional_tip = if COMMON_COMMANDS.contains(&var) {
            format!(" (for output from commands, use \"$({} ...)\" )", var)
        } else {
            match get_best_match(var) {
                Some(m) => format!(" (did you mean '{}'?)", m),
                None => String::new(),
            }
        };
        Some(make_comment(
            Severity::WarningC,
            place,
            2154,
            format!("{} is referenced but not assigned{}.", var, optional_tip),
        ))
    };

    // ${foo[bar baz]} may not be referencing bar/baz. Just skip these.
    // We can also have ${foo:+$foo} should be treated like [[ -n $foo ]] && echo $foo
    let is_exception = |var: &str, t: Id| -> bool {
        get_path(ast, t).into_iter().any(|t| match ast.get(t) {
            Token::T_DollarBraced(_, l) => {
                let str = oversimplify(ast, *l).concat();
                let r = get_braced_reference(&str);
                let m = get_braced_modifier(&str);
                // Either we're used as an array index like ${arr[here]}
                r != var
                    // or the reference is guarded by a parent, ${here:+foo$here}
                    || m.starts_with('+')
                    || m.starts_with(":+")
            }
            _ => false,
        })
    };

    let is_guarded = |t: Id| -> bool {
        match ast.get(t) {
            Token::T_DollarBraced(_, v) => {
                let name = oversimplify(ast, *v).concat();
                let rest: String = name
                    .trim_start_matches(|c| c == '#' || c == '!')
                    .trim_start_matches(is_variable_char)
                    .to_string();
                matches(&rest, &GUARD_REGEX)
            }
            _ => false,
        }
    };

    let warning_for = |var: &str, place: Id| -> Option<TokenComment> {
        if !is_variable_name(var) {
            return None;
        }
        if is_exception(var, place) || is_guarded(place) {
            return None;
        }
        if include_globals || is_local(var) {
            warning_for_locals(var, place)
        } else {
            warning_for_globals(var, place)
        }
    };

    unassigned.into_iter().filter_map(|(var, place)| warning_for(var, place)).collect()
}
