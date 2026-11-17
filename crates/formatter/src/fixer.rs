// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the parts of ShellCheck src/ShellCheck/Fixer.hs (e11f964) that the
// formatters need: removeTabStops, mapPositions, the Fix semigroup and applyFix.
//
// DIVERGENCE: Fixer.hs is otherwise out of scope for Phase 1 (Task 17); this
// private subset exists because Format.hs and TTY.hs call into it. The
// prefix-sum tree is replaced by a flat list with identical semantics
// (sum of the values whose key is <= target).

use shelllint_core::interface::{Fix, InsertionPoint, Position, PositionedComment, Replacement};

fn overlap(xs: &Position, xe: &Position, ys: &Position, ye: &Position) -> bool {
    xe > ys && ye > xs
}

fn real(line: impl Iterator<Item = char>, target: i64) -> i64 {
    // real _ r v target | target <= v = r
    // real [] r v target = r + (target - v)
    let (mut r, mut v) = (0i64, 0i64);
    let mut it = line;
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

/// The lines of a file, each with a flag for whether it holds a tab. A column
/// is only moved by a tab before it, so a line without one is never walked.
/// Build it once per file: the flags cost one scan of the file.
pub struct FileLines<'a> {
    pub lines: Vec<&'a str>,
    tabbed: Vec<bool>,
}

impl<'a> FileLines<'a> {
    pub fn new(lines: Vec<&'a str>) -> Self {
        let tabbed = lines.iter().map(|l| l.contains('\t')).collect();
        FileLines { lines, tabbed }
    }
}

fn realign_column(ls: &FileLines, line: i64, col: i64) -> i64 {
    if line > 0 && line <= ls.lines.len() as i64 {
        let index = (line - 1) as usize;
        if ls.tabbed[index] {
            real(ls.lines[index].chars(), col)
        } else {
            // With no tabs `real` gives `col` when it is positive, and 0 otherwise.
            col.max(0)
        }
    } else {
        col
    }
}

fn realigned(ls: &FileLines, start: &Position, end: &Position) -> (Position, Position) {
    let sc = realign_column(ls, start.line, start.column);
    let ec = realign_column(ls, end.line, end.column);
    (Position { column: sc, ..start.clone() }, Position { column: ec, ..end.clone() })
}

/// Rewrite a comment from a tabstop of 8 to 1
pub fn remove_tab_stops_comment(c: &PositionedComment, ls: &FileLines) -> PositionedComment {
    let (s, e) = realigned(ls, &c.start_pos, &c.end_pos);
    PositionedComment { start_pos: s, end_pos: e, ..c.clone() }
}

/// Rewrite a replacement from a tabstop of 8 to 1
pub fn remove_tab_stops_replacement(r: &Replacement, ls: &FileLines) -> Replacement {
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
    let tab_lines = FileLines::new(file_lines.to_vec());
    let untabbed: Vec<Replacement> =
        fix.replacements.iter().map(|r| remove_tab_stops_replacement(r, &tab_lines)).collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use shelllint_core::interface::new_positioned_comment;

    fn pos(line: i64, column: i64) -> Position {
        Position { file: "f".into(), line, column }
    }

    #[test]
    fn tab_free_line_realigns_to_the_column_itself() {
        // The walk `real` gives the column back once it is positive, and 0 otherwise.
        let lines = FileLines::new(vec!["echo $a $b", ""]);
        for col in [-3, 0, 1, 5, 10, 11, 12, 40] {
            let walked = real("echo $a $b".chars(), col);
            assert_eq!(realign_column(&lines, 1, col), walked, "column {col} on a tab-free line");
            assert_eq!(realign_column(&lines, 2, col), real("".chars(), col), "column {col} on an empty line");
        }
    }

    #[test]
    fn tabbed_line_still_walks_tab_stops() {
        // A tab at column 1 jumps to 8, so column 9 is character 2 after the tab.
        let lines = FileLines::new(vec!["\tx y"]);
        assert_eq!(realign_column(&lines, 1, 9), 2);
        assert_eq!(realign_column(&lines, 1, 2), 1);
        assert_eq!(realign_column(&lines, 1, 0), 0);
    }

    #[test]
    fn lines_outside_the_file_keep_their_column() {
        let lines = FileLines::new(vec!["a"]);
        assert_eq!(realign_column(&lines, 0, 7), 7);
        assert_eq!(realign_column(&lines, 2, 7), 7);
    }

    #[test]
    fn remove_tab_stops_comment_realigns_both_ends() {
        let lines = FileLines::new(vec!["\tab"]);
        let c = PositionedComment { start_pos: pos(1, 9), end_pos: pos(1, 10), ..new_positioned_comment() };
        let out = remove_tab_stops_comment(&c, &lines);
        assert_eq!((out.start_pos.column, out.end_pos.column), (2, 3));
    }

    /// 20000 comments on one 60 KB line (a long SC2086 line): realigning each
    /// used to copy the line into a `Vec<char>`, which made the whole run
    /// quadratic in the line length.
    #[test]
    fn many_comments_on_one_long_line_realign_in_time() {
        let line = "a=1; echo ".to_string() + &"$a ".repeat(20_000);
        let lines = FileLines::new(vec![line.as_str()]);
        let start = std::time::Instant::now();
        for i in 0..20_000i64 {
            let c = PositionedComment {
                start_pos: pos(1, 10 + 3 * i),
                end_pos: pos(1, 12 + 3 * i),
                ..new_positioned_comment()
            };
            let out = remove_tab_stops_comment(&c, &lines);
            assert_eq!(out.start_pos.column, 10 + 3 * i);
        }
        let elapsed = start.elapsed();
        assert!(elapsed < std::time::Duration::from_secs(5), "took {elapsed:?}");
    }
}
