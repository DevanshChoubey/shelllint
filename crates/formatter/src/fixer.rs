// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the parts of ShellCheck src/ShellCheck/Fixer.hs (e11f964) that the
// formatters need: removeTabStops, mapPositions, the Fix semigroup and applyFix.
//
// DIVERGENCE: Fixer.hs is otherwise out of scope for Phase 1 (Task 17); this
// private subset exists because Format.hs and TTY.hs call into it. The
// prefix-sum tree is replaced by a flat list with identical semantics
// (sum of the values whose key is <= target).

use shelllint_core::interface::{Fix, InsertionPoint, Position, PositionedComment, Replacement};

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

/// Accepted replacements, indexed so that an overlap query takes O(log R).
///
/// Two replacements overlap when each one starts before the other ends. The
/// positions are ranked once, in `Position` order, so ranks and comparisons
/// agree. The Fenwick tree is over start ranks and stores the largest end rank
/// (plus one) of an accepted replacement starting there, so a prefix maximum
/// answers "is there an accepted replacement that starts before this end and
/// ends after this start".
struct OverlapIndex {
    keys: Vec<Position>,
    tree: Vec<usize>,
}

impl OverlapIndex {
    fn new(mut keys: Vec<Position>) -> Self {
        keys.sort();
        keys.dedup();
        let n = keys.len();
        OverlapIndex { keys, tree: vec![0; n + 1] }
    }

    fn rank(&self, p: &Position) -> usize {
        self.keys.binary_search(p).expect("every position is registered")
    }

    fn insert(&mut self, start: &Position, end: &Position) {
        let value = self.rank(end) + 1;
        let mut i = self.rank(start) + 1;
        while i < self.tree.len() {
            self.tree[i] = self.tree[i].max(value);
            i += i & i.wrapping_neg();
        }
    }

    /// Whether an accepted replacement overlaps `start..end`.
    fn overlaps(&self, start: &Position, end: &Position) -> bool {
        // Accepted starts before `end` have a rank below this count.
        let mut m = self.keys.partition_point(|k| k < end);
        let mut best = 0;
        while m > 0 {
            best = best.max(self.tree[m]);
            m -= m & m.wrapping_neg();
        }
        best > self.rank(start) + 1
    }
}

/// The Semigroup instance for Fix, folded over `fixes` in order: a fix that
/// overlaps an earlier accepted replacement is dropped whole, as `fix_append`
/// keeps the left side on overlap. Each fix is checked against every accepted
/// replacement at once, not pairwise.
pub fn merge_fixes(fixes: &[Fix]) -> Fix {
    let keys: Vec<Position> = fixes
        .iter()
        .flat_map(|f| f.replacements.iter())
        .flat_map(|r| [r.start_pos.clone(), r.end_pos.clone()])
        .collect();
    let mut index = OverlapIndex::new(keys);
    let mut accepted: Vec<Replacement> = vec![];
    for fix in fixes {
        // Check the whole fix before adding any of it, as the fold does.
        let conflict = fix.replacements.iter().any(|r| index.overlaps(&r.start_pos, &r.end_pos));
        if !conflict {
            for r in &fix.replacements {
                index.insert(&r.start_pos, &r.end_pos);
                accepted.push(r.clone());
            }
        }
    }
    Fix { replacements: accepted }
}

/// Values keyed by integer, with prefix sums over the keys. The keys are known
/// before any value is added; an update or a query takes O(log K).
struct SumTree {
    keys: Vec<i64>,
    tree: Vec<i64>,
}

impl SumTree {
    fn new(mut keys: Vec<i64>) -> Self {
        keys.sort_unstable();
        keys.dedup();
        let n = keys.len();
        SumTree { keys, tree: vec![0; n + 1] }
    }

    fn add(&mut self, key: i64, value: i64) {
        let mut i = self.keys.binary_search(&key).expect("key registered") + 1;
        while i < self.tree.len() {
            self.tree[i] += value;
            i += i & i.wrapping_neg();
        }
    }

    /// The sum of the values whose key is `<= target`.
    fn sum_upto(&self, target: i64) -> i64 {
        let mut m = self.keys.partition_point(|k| *k <= target);
        let mut sum = 0;
        while m > 0 {
            sum += self.tree[m];
            m -= m & m.wrapping_neg();
        }
        sum
    }
}

/// The text being edited, as blocks of chars, so that a replacement costs about
/// one block and not a copy of the whole file.
struct EditText {
    blocks: Vec<Vec<char>>,
    total: usize,
}

const EDIT_BLOCK: usize = 256;

impl EditText {
    fn new(chars: &[char]) -> Self {
        let blocks = chars.chunks(EDIT_BLOCK).map(<[char]>::to_vec).collect();
        EditText { blocks, total: chars.len() }
    }

    /// The block holding char index `at`, and the offset in it. At the end of
    /// the text this is past the last block.
    fn locate(&self, at: usize) -> (usize, usize) {
        let mut rest = at;
        for (bi, b) in self.blocks.iter().enumerate() {
            if rest < b.len() {
                return (bi, rest);
            }
            rest -= b.len();
        }
        (self.blocks.len(), 0)
    }

    /// Remove `delete` chars from index `at`, then insert `text` there.
    fn splice(&mut self, at: usize, delete: usize, text: &str) {
        let mut left = delete;
        let (mut bi, mut off) = self.locate(at);
        while left > 0 && bi < self.blocks.len() {
            let b = &mut self.blocks[bi];
            let n = left.min(b.len() - off);
            b.drain(off..off + n);
            left -= n;
            self.total -= n;
            if b.is_empty() {
                self.blocks.remove(bi);
            } else {
                bi += 1;
            }
            off = 0;
        }
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return;
        }
        self.total += chars.len();
        let (bi, off) = self.locate(at);
        if bi == self.blocks.len() {
            self.blocks.push(chars);
        } else {
            self.blocks[bi].splice(off..off, chars);
            if self.blocks[bi].len() > 2 * EDIT_BLOCK {
                let big = self.blocks.remove(bi);
                let parts: Vec<Vec<char>> = big.chunks(EDIT_BLOCK).map(<[char]>::to_vec).collect();
                self.blocks.splice(bi..bi, parts);
            }
        }
    }

    /// Replace the chars from 1-based `start` up to `end` (exclusive) with `r`.
    /// Positions past either end are clamped, as the Haskell `replace` does.
    fn replace(&mut self, start: i64, end: i64, r: &str) {
        let (si, ei) = (start - 1, end - 1);
        let at = si.clamp(0, self.total as i64) as usize;
        let delete = ((ei - si).max(0) as usize).min(self.total - at);
        self.splice(at, delete, r);
    }

    fn into_string(self) -> String {
        self.blocks.iter().flatten().collect()
    }
}

/// Apply a fix and return resulting lines.
pub fn apply_fix(fix: &Fix, file_lines: &[&str]) -> Vec<String> {
    let tab_lines = FileLines::new(file_lines.to_vec());
    let untabbed: Vec<Replacement> =
        fix.replacements.iter().map(|r| remove_tab_stops_replacement(r, &tab_lines)).collect();

    // multiToSingleLine: a line's columns move by the length of every line
    // before it, plus one newline each.
    let mut line_shift = SumTree::new((0..file_lines.len()).map(|i| i as i64 + 2).collect());
    for (i, s) in file_lines.iter().enumerate() {
        line_shift.add(i as i64 + 2, s.chars().count() as i64 + 1);
    }
    let mut single: Vec<char> = vec![];
    for s in file_lines {
        single.extend(s.chars());
        single.push('\n');
    }
    let adjust = |pos: &Position| Position {
        file: pos.file.clone(),
        line: 1,
        column: pos.column + line_shift.sum_upto(pos.line),
    };
    let adjusted: Vec<Replacement> = untabbed
        .iter()
        .map(|r| Replacement { start_pos: adjust(&r.start_pos), end_pos: adjust(&r.end_pos), ..r.clone() })
        .collect();

    // applyReplacements2: reverse $ sortWith repPrecedence
    let mut reps = adjusted;
    reps.sort_by_key(|r| r.precedence);
    reps.reverse();

    // Each replacement adds an offset at its insertion point. Those points come
    // from the adjusted columns alone, so they are all known before the loop.
    let insertion_point = |rep: &Replacement| match rep.insertion_point {
        InsertionPoint::InsertBefore => rep.start_pos.column,
        InsertionPoint::InsertAfter => rep.end_pos.column + 1,
    };
    let mut offsets = SumTree::new(reps.iter().map(insertion_point).collect());
    let mut text = EditText::new(&single);
    for rep in &reps {
        let (old_start, old_end) = (rep.start_pos.column, rep.end_pos.column);
        let new_start = old_start + offsets.sum_upto(old_start);
        let new_end = old_end + offsets.sum_upto(old_end);
        let shift = rep.string.chars().count() as i64 - (old_end - old_start);
        offsets.add(insertion_point(rep), shift);
        text.replace(new_start, new_end, &rep.string);
    }
    let result = text.into_string();
    crate::haskell_lines(&result).into_iter().map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shelllint_core::interface::new_positioned_comment;

    fn pos(line: i64, column: i64) -> Position {
        Position { file: "f".into(), line, column }
    }

    /// The reference: copy the whole text for each replacement.
    fn naive_replace(o: &[char], start: i64, end: i64, r: &str) -> Vec<char> {
        let si = start - 1;
        let ei = end - 1;
        let n = o.len() as i64;
        let split = si.clamp(0, n) as usize;
        let drop_n = (ei - si).max(0) as usize;
        let mut out: Vec<char> = o[..split].to_vec();
        out.extend(r.chars());
        out.extend(o[split..].iter().skip(drop_n));
        out
    }

    /// `EditText` gives the same text as the whole-copy replace, over many edits
    /// that cross block boundaries, delete past the end and insert long text.
    #[test]
    fn edit_text_matches_the_whole_copy() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for _ in 0..200 {
            let base: Vec<char> = (0..next(2000)).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
            let mut text = EditText::new(&base);
            let mut naive = base.clone();
            for _ in 0..60 {
                let len = naive.len() as u64;
                let start = next(len + 3) as i64 - 1;
                let end = start + next(40) as i64 - 3;
                let ins: String = "XYZ-é".chars().cycle().take(next(700) as usize).collect();
                text.replace(start, end, &ins);
                naive = naive_replace(&naive, start, end, &ins);
                assert_eq!(text.total, naive.len());
            }
            let got: String = text.into_string();
            assert_eq!(got, naive.iter().collect::<String>());
        }
    }

    fn rep_at(line: i64, start: i64, end: i64, text: &str, precedence: i64) -> Replacement {
        Replacement {
            start_pos: pos(line, start),
            end_pos: pos(line, end),
            string: text.to_string(),
            precedence,
            insertion_point: InsertionPoint::InsertBefore,
        }
    }

    /// The reference: fold `fix_append` over the fixes in order, pairwise.
    fn naive_merge(fixes: &[Fix]) -> Fix {
        fn overlap(xs: &Position, xe: &Position, ys: &Position, ye: &Position) -> bool {
            xe > ys && ye > xs
        }
        fixes.iter().fold(Fix { replacements: vec![] }, |f1, f2| {
            let conflict = f1.replacements.iter().any(|r1| {
                f2.replacements.iter().any(|r2| overlap(&r2.start_pos, &r2.end_pos, &r1.start_pos, &r1.end_pos))
            });
            if conflict {
                f1
            } else {
                let mut v = f1.replacements;
                v.extend(f2.replacements.iter().cloned());
                Fix { replacements: v }
            }
        })
    }

    /// `merge_fixes` keeps the same fixes, in the same order, as the pairwise fold,
    /// on many small random sets with touching, empty and overlapping ranges.
    #[test]
    fn merge_fixes_matches_the_pairwise_fold() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for _ in 0..2000 {
            let fixes: Vec<Fix> = (0..1 + next(6))
                .map(|_| {
                    let replacements = (0..1 + next(4))
                        .map(|_| {
                            let line = 1 + next(2) as i64;
                            let start = next(12) as i64;
                            let end = start + next(5) as i64;
                            rep_at(line, start, end, "x", next(3) as i64)
                        })
                        .collect();
                    Fix { replacements }
                })
                .collect();
            assert_eq!(merge_fixes(&fixes), naive_merge(&fixes), "fixes {fixes:?}");
        }
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
