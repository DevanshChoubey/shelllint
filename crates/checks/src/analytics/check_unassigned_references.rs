// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnassignedReferences`, lines 2649-2649 (e11f964)

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use shelllint_analyzer::analyzerlib::{Params, StackData, make_comment};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    get_braced_modifier, get_braced_reference, get_path, is_variable_char, is_variable_name, oversimplify,
};
use shelllint_core::data::{COMMON_COMMANDS, INTERNAL_VARIABLES};
use shelllint_core::interface::{Severity, TokenComment};
use shelllint_core::regex::{matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnassignedReferences",
    registry: "tree",
    order: 8,
    optional: None,
    kind: CheckKind::Tree(check_unassigned_references),
};

pub(crate) static ENTRY_TRUE: CheckEntry = CheckEntry {
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

// `dist` is the wiki's edit-distance implementation; any Levenshtein
// distance computes the same value. `getBestMatch` only accepts a best score
// of at most 2, so scores are computed capped at 3 (`MATCH_CAP`): the first
// candidate with the minimal capped score is the first with the minimal
// score whenever that minimum is at most 2, and otherwise both give
// Nothing.
const MATCH_CAP: usize = 3;

/// `min (dist a b) cap`, using two reusable rows and stopping once a whole
/// row is at least `cap` (row minima never decrease).
fn dist_capped(a: &[char], b: &[char], cap: usize, prev: &mut Vec<usize>, cur: &mut Vec<usize>) -> usize {
    if a.len().abs_diff(b.len()) >= cap {
        return cap;
    }
    prev.clear();
    prev.extend(0..=b.len());
    for (i, ac) in a.iter().enumerate() {
        cur.clear();
        cur.push(i + 1);
        for (j, bc) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ac != bc);
            let v = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
            cur.push(v);
        }
        if cur.iter().all(|&v| v >= cap) {
            return cap;
        }
        std::mem::swap(prev, cur);
    }
    prev[b.len()].min(cap)
}

fn to_lower(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// A name with its characters and `map toLower` characters, computed once.
struct Candidate<'a> {
    name: &'a str,
    chars: Vec<char>,
    lower: Vec<char>,
}

impl<'a> Candidate<'a> {
    fn new(name: &'a str) -> Self {
        Candidate { name, chars: name.chars().collect(), lower: to_lower(name) }
    }
}

/// `min (match var candidate) MATCH_CAP`, where
/// `match var candidate = if var /= candidate && map toLower var == map toLower candidate
///                        then 1 else dist var candidate`
fn match_capped(var: &Candidate<'_>, candidate: &Candidate<'_>, prev: &mut Vec<usize>, cur: &mut Vec<usize>) -> usize {
    if var.name != candidate.name && var.lower == candidate.lower {
        1
    } else {
        dist_capped(&var.chars, &candidate.chars, MATCH_CAP, prev, cur)
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
    let written_vars: Vec<Candidate<'_>> =
        write_map.keys().filter(|k| is_variable_name(k)).map(|k| Candidate::new(k)).collect();

    let get_best_match = |var: &str| -> Option<String> {
        // `listToMaybe (sortBy (comparing snd) matches)`: the first candidate
        // with the minimal score (sortBy is stable).
        let var = Candidate::new(var);
        let (mut prev, mut cur) = (Vec::new(), Vec::new());
        let mut best: Option<(&Candidate<'_>, usize)> = None;
        for x in &written_vars {
            let score = match_capped(&var, x, &mut prev, &mut cur);
            if best.is_none_or(|(_, s)| score < s) {
                best = Some((x, score));
            }
        }
        let (m, score) = best?;
        let l = m.chars.len();
        let good = l > 3 && score <= 1 || l > 7 && score <= 2;
        if good { Some(m.name.to_string()) } else { None }
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
                    .trim_start_matches(['#', '!'])
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
