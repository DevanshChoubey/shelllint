// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the parts of ShellCheck src/ShellCheck/Fixer.hs (e11f964) that the
// formatters need: removeTabStops, mapPositions, the Fix semigroup and applyFix.
//
// DIVERGENCE: Fixer.hs is otherwise out of scope for Phase 1 (Task 17); this
// private subset exists because Format.hs and TTY.hs call into it. The
// prefix-sum tree is replaced by a flat list with identical semantics
// (sum of the values whose key is <= target).

use shellcheck_core::interface::{Fix, InsertionPoint, Position, PositionedComment, Replacement};

fn overlap(xs: &Position, xe: &Position, ys: &Position, ye: &Position) -> bool {
    xe > ys && ye > xs
}

fn real(line: &[char], target: i64) -> i64 {
    // real _ r v target | target <= v = r
    // real [] r v target = r + (target - v)
    let (mut r, mut v) = (0i64, 0i64);
    let mut it = line.iter();
    loop {
        if target <= v {
            return r;
        }
        match it.next() {
            None => return r + (target - v),
            Some('\t') => {
                r += 1;
                v += 8 - (v % 8);
            }
            Some(_) => {
                r += 1;
                v += 1;
            }
        }
    }
}

fn realign_column(ls: &[&str], line: i64, col: i64) -> i64 {
    if line > 0 && line <= ls.len() as i64 {
        let chars: Vec<char> = ls[(line - 1) as usize].chars().collect();
        real(&chars, col)
    } else {
        col
    }
}

fn realigned(ls: &[&str], start: &Position, end: &Position) -> (Position, Position) {
    let sc = realign_column(ls, start.line, start.column);
    let ec = realign_column(ls, end.line, end.column);
    (Position { column: sc, ..start.clone() }, Position { column: ec, ..end.clone() })
}

/// Rewrite a comment from a tabstop of 8 to 1
pub fn remove_tab_stops_comment(c: &PositionedComment, ls: &[&str]) -> PositionedComment {
    let (s, e) = realigned(ls, &c.start_pos, &c.end_pos);
    PositionedComment { start_pos: s, end_pos: e, ..c.clone() }
}

/// Rewrite a replacement from a tabstop of 8 to 1
pub fn remove_tab_stops_replacement(r: &Replacement, ls: &[&str]) -> Replacement {
    let (s, e) = realigned(ls, &r.start_pos, &r.end_pos);
    Replacement { start_pos: s, end_pos: e, ..r.clone() }
}

pub fn map_positions(f: impl Fn(&Position) -> Position, fix: &Fix) -> Fix {
    Fix {
        replacements: fix
            .replacements
            .iter()
            .map(|r| Replacement { start_pos: f(&r.start_pos), end_pos: f(&r.end_pos), ..r.clone() })
            .collect(),
    }
}

/// The Semigroup instance for Fix: discard the right side on overlap.
fn fix_append(f1: Fix, f2: &Fix) -> Fix {
    let conflict = f1
        .replacements
        .iter()
        .any(|r1| f2.replacements.iter().any(|r2| overlap(&r2.start_pos, &r2.end_pos, &r1.start_pos, &r1.end_pos)));
    if conflict {
        f1
    } else {
        let mut v = f1.replacements;
        v.extend(f2.replacements.iter().cloned());
        Fix { replacements: v }
    }
}

/// `fold fixes`: mconcat = foldl mappend mempty
pub fn merge_fixes(fixes: &[Fix]) -> Fix {
    fixes.iter().fold(Fix { replacements: vec![] }, fix_append)
}

fn get_prefix_sum(target: i64, tree: &[(i64, i64)]) -> i64 {
    tree.iter().filter(|(k, _)| *k <= target).map(|(_, v)| v).sum()
}

fn add_ps_value(key: i64, value: i64, tree: &mut Vec<(i64, i64)>) {
    if value != 0 {
        tree.push((key, value));
    }
}

fn do_replace(start: i64, end: i64, o: &[char], r: &str) -> Vec<char> {
    let si = start - 1;
    let ei = end - 1;
    let n = o.len() as i64;
    let split = si.clamp(0, n) as usize;
    let drop_n = (ei - si).max(0) as usize;
    let mut out: Vec<char> = o[..split].to_vec();
    out.extend(r.chars());
    let rest = &o[split..];
    out.extend(rest.iter().skip(drop_n));
    out
}

/// Apply a fix and return resulting lines.
pub fn apply_fix(fix: &Fix, file_lines: &[&str]) -> Vec<String> {
    let untabbed: Vec<Replacement> =
        fix.replacements.iter().map(|r| remove_tab_stops_replacement(r, file_lines)).collect();

    // multiToSingleLine
    let mut shift_tree: Vec<(i64, i64)> = vec![];
    for (i, s) in file_lines.iter().enumerate() {
        add_ps_value(i as i64 + 2, s.chars().count() as i64 + 1, &mut shift_tree);
    }
    let mut single: Vec<char> = vec![];
    for s in file_lines {
        single.extend(s.chars());
        single.push('\n');
    }
    let adjust = |pos: &Position| Position {
        file: pos.file.clone(),
        line: 1,
        column: pos.column + get_prefix_sum(pos.line, &shift_tree),
    };
    let adjusted: Vec<Replacement> = untabbed
        .iter()
        .map(|r| Replacement { start_pos: adjust(&r.start_pos), end_pos: adjust(&r.end_pos), ..r.clone() })
        .collect();

    // applyReplacements2: reverse $ sortWith repPrecedence
    let mut reps = adjusted;
    reps.sort_by_key(|r| r.precedence);
    reps.reverse();
    let mut tree: Vec<(i64, i64)> = vec![];
    let mut string = single;
    for rep in &reps {
        let (old_start, old_end) = (rep.start_pos.column, rep.end_pos.column);
        let new_start = old_start + get_prefix_sum(old_start, &tree);
        let new_end = old_end + get_prefix_sum(old_end, &tree);
        let shift = rep.string.chars().count() as i64 - (old_end - old_start);
        let insertion_point = match rep.insertion_point {
            InsertionPoint::InsertBefore => old_start,
            InsertionPoint::InsertAfter => old_end + 1,
        };
        add_ps_value(insertion_point, shift, &mut tree);
        string = do_replace(new_start, new_end, &string, &rep.string);
    }
    let result: String = string.into_iter().collect();
    crate::haskell_lines(&result).into_iter().map(String::from).collect()
}
