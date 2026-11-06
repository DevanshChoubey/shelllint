// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/Diff.hs (e11f964)
//
// `Data.Algorithm.Diff.getDiff` from the Diff package is ported below.
//
// DIVERGENCE: `a </> name` is only implemented for POSIX paths (an absolute
// `name` replaces the `a` prefix); Windows drive/separator handling is not.

use std::collections::BTreeMap;
use std::io::Write;

use shellcheck_core::interface::{CheckResult, Fix, FormatterOptions, SystemInterface};

use crate::fixer::{apply_fix, merge_fixes};
use crate::*;

const CONTEXT_SIZE: usize = 3;
const RED: i32 = 31;
const GREEN: i32 = 32;
const CYAN: i32 = 36;
const BOLD: i32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diff<T> {
    Both(T, T),
    First(T),
    Second(T),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfStatus {
    LinefeedMissing,
    LinefeedOk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRegion<T> {
    pub left: (i64, i64),
    pub right: (i64, i64),
    pub diffs: Vec<Diff<T>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffDoc<T> {
    pub name: String,
    pub lf: LfStatus,
    pub regions: Vec<DiffRegion<T>>,
}

pub struct DiffFormatter {
    found_issues: bool,
    reported_issues: bool,
    color: bool,
}

pub fn format(options: FormatterOptions) -> DiffFormatter {
    DiffFormatter { found_issues: false, reported_issues: false, color: should_output_color(options.color_option) }
}

fn ansi(n: i32) -> String {
    format!("\x1B[{}m", n)
}

fn colorize(on: bool, n: i32, s: &str) -> String {
    if on { format!("{}{}{}", ansi(n), s, ansi(0)) } else { s.to_string() }
}

fn print_err(color: bool, msg: &str) {
    eprintln!("{}", colorize(color, BOLD, &colorize(color, RED, msg)));
}

impl Formatter for DiffFormatter {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn footer(&mut self, _out: &mut dyn Write) {
        if self.found_issues && !self.reported_issues {
            print_err(self.color, "Issues were detected, but none were auto-fixable. Use another format to see them.");
        }
    }

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        print_err(self.color, &format!("{}: {}", file, msg));
    }

    fn on_result(&mut self, result: &CheckResult, sys: &dyn SystemInterface, out: &mut dyn Write) {
        let comments = &result.comments;
        if !comments.is_empty() {
            self.found_issues = true;
        }
        let fixmap = fixes_by_file(result);
        for (name, fix) in fixmap {
            match sys.read_file(Some(true), &name) {
                Ok(contents) => {
                    let doc = make_diff(&name, &contents, &fix);
                    // putStrLn: formatDoc output plus one more newline
                    let _ = writeln!(out, "{}", format_doc(self.color, &doc));
                    self.reported_issues = true;
                }
                Err(msg) => self.on_failure(&name, &msg, out),
            }
        }
    }
}

fn has_trailing_linefeed(s: &str) -> bool {
    s.is_empty() || s.ends_with('\n')
}

fn covers_last_line<T: Clone>(regions: &[(bool, Vec<Diff<T>>)]) -> bool {
    regions.last().map(|r| r.0).unwrap_or(false)
}

pub fn make_diff(name: &str, contents: &str, fix: &Fix) -> DiffDoc<String> {
    let hunks = group_diff(&compute_diff(contents, fix));
    let lf = if covers_last_line(&hunks) && !has_trailing_linefeed(contents) {
        LfStatus::LinefeedMissing
    } else {
        LfStatus::LinefeedOk
    };
    DiffDoc { name: name.to_string(), lf, regions: find_regions(hunks) }
}

fn compute_diff(contents: &str, fix: &Fix) -> Vec<Diff<String>> {
    let old = haskell_lines(contents);
    let new = apply_fix(fix, &old);
    let old: Vec<String> = old.into_iter().map(String::from).collect();
    get_diff(&old, &new)
}

// ---- Data.Algorithm.Diff (package Diff >= 0.4) ------------------------------
//
// A port of `getDiff`: the O(ND) algorithm of Myers' "An O(ND) Difference
// Algorithm and Its Variations", as the Haskell writes it, including its
// `Ord L` instance and `pairMaxes`, which decide between equally short edit
// scripts:
//
//   data L = L {poi :: !Int, poj :: !Int, path :: [DI]}
//   instance Ord L where x <= y = if poi x == poi y then poj x > poj y
//                                                   else poi x <= poi y
//   dstep cd dls = hd:pairMaxes rst
//     where (hd:rst) = nextDLs dls
//           nextDLs (dl:rest) = dl':dl'':nextDLs rest
//             where dl'  = addsnake cd $ dl {poi=poi dl + 1, path=(F : pdl)}
//                   dl'' = addsnake cd $ dl {poj=poj dl + 1, path=(S : pdl)}
//   lcs cd lena lenb = path . head . dropWhile (\dl -> poi dl /= lena || poj dl /= lenb) .
//               concat . iterate (dstep cd) . (:[]) . addsnake cd $ L {poi=0,poj=0,path=[]}
//
// So element m (0..=n) of step d+1 is `F` of element m of step d (m = 0),
// `max (S of m-1) (F of m)`, or `S` of element n-1 (m = n).
//
// DIVERGENCE: instead of carrying each `path` (a cons list the GC shares
// and reclaims), every step records each element's `poi` and whether it
// came from an `S`; the winning path is rebuilt by walking back. The edit
// script is the same; memory is 5 bytes per frontier element and step.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Di {
    F,
    S,
    /// `n` consecutive `B`s (one snake).
    B(usize),
}

/// `x <= y` of `instance Ord L`, on (poi, poj).
fn l_le(x: (usize, usize), y: (usize, usize)) -> bool {
    if x.0 == y.0 { x.1 > y.1 } else { x.0 <= y.0 }
}

/// `lcs`: the path of the first `L`, in `concat . iterate (dstep cd)` order,
/// that reaches (lena, lenb), in chronological order.
fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Di> {
    // `addsnake cd`, with `canDiag`'s bounds checks.
    let addsnake = |(mut i, mut j): (usize, usize)| {
        while i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        }
        (i, j)
    };
    let start = addsnake((0, 0));
    let mut frontier: Vec<(usize, usize)> = vec![start];
    let mut trace_poi: Vec<Vec<u32>> = vec![vec![start.0 as u32]];
    let mut trace_s: Vec<Vec<bool>> = vec![vec![false]];
    let (mut d, mut m) = loop {
        if let Some(m) = frontier.iter().position(|&(i, j)| i == a.len() && j == b.len()) {
            break (trace_poi.len() - 1, m);
        }
        // dstep
        let n = frontier.len();
        let mut next = Vec::with_capacity(n + 1);
        let mut pois = Vec::with_capacity(n + 1);
        let mut from_s = Vec::with_capacity(n + 1);
        let f = |(i, j): (usize, usize)| addsnake((i + 1, j));
        let s = |(i, j): (usize, usize)| addsnake((i, j + 1));
        let mut push = |l: (usize, usize), is_s: bool| {
            next.push(l);
            pois.push(l.0 as u32);
            from_s.push(is_s);
        };
        push(f(frontier[0]), false);
        for k in 1..n {
            // pairMaxes: max x y = if x <= y then y else x
            let x = s(frontier[k - 1]);
            let y = f(frontier[k]);
            if l_le(x, y) { push(y, false) } else { push(x, true) }
        }
        push(s(frontier[n - 1]), true);
        frontier = next;
        trace_poi.push(pois);
        trace_s.push(from_s);
    };
    drop(frontier);
    // Walk back from (d, m), emitting each step's snake, then its F or S.
    let mut steps = vec![];
    let mut cur = trace_poi[d][m] as usize;
    while d > 0 {
        let is_s = trace_s[d][m];
        let parent_m = if is_s { m - 1 } else { m };
        let parent = trace_poi[d - 1][parent_m] as usize;
        let snake = cur - (parent + usize::from(!is_s));
        if snake > 0 {
            steps.push(Di::B(snake));
        }
        steps.push(if is_s { Di::S } else { Di::F });
        d -= 1;
        m = parent_m;
        cur = parent;
    }
    if cur > 0 {
        steps.push(Di::B(cur));
    }
    steps.reverse();
    steps
}

/// Data.Algorithm.Diff `getDiff` (`getDiffBy (==)`): `markup a b` of the
/// reversed `lcs` path.
pub fn get_diff<T: Clone + PartialEq>(a: &[T], b: &[T]) -> Vec<Diff<T>> {
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    let (mut i, mut j) = (0, 0);
    for step in lcs(a, b) {
        match step {
            Di::F => {
                out.push(Diff::First(a[i].clone()));
                i += 1;
            }
            Di::S => {
                out.push(Diff::Second(b[j].clone()));
                j += 1;
            }
            Di::B(n) => {
                for _ in 0..n {
                    out.push(Diff::Both(a[i].clone(), b[j].clone()));
                    i += 1;
                    j += 1;
                }
            }
        }
    }
    out
}

fn is_both<T>(d: &Diff<T>) -> bool {
    matches!(d, Diff::Both(..))
}

/// Group changes into hunks. The bool says whether the hunk is output.
pub fn group_diff<T: Clone>(diffs: &[Diff<T>]) -> Vec<(bool, Vec<Diff<T>>)> {
    // `cur` holds the pending run in chronological order (the Haskell
    // accumulator is the reverse of this).
    let mut result: Vec<(bool, Vec<Diff<T>>)> = vec![];
    let mut cur: Vec<Diff<T>> = vec![];
    let mut i = 0;
    let len = diffs.len();
    // hunt phase
    'outer: loop {
        // hunt
        while i < len && is_both(&diffs[i]) {
            cur.push(diffs[i].clone());
            i += 1;
        }
        if i >= len {
            result.push((false, cur));
            break;
        }
        let split = cur.len().saturating_sub(CONTEXT_SIZE);
        let context = cur.split_off(split);
        result.push((false, std::mem::take(&mut cur)));
        cur = context;
        // gather
        let mut n = 0usize;
        loop {
            if i >= len {
                let k = n.saturating_sub(CONTEXT_SIZE).min(cur.len());
                let extras = cur.split_off(cur.len() - k);
                result.push((true, cur));
                result.push((false, extras));
                break 'outer;
            }
            if is_both(&diffs[i]) {
                if n == CONTEXT_SIZE * 2 {
                    let split = cur.len().saturating_sub(CONTEXT_SIZE);
                    let context = cur.split_off(split);
                    result.push((true, std::mem::take(&mut cur)));
                    cur = context;
                    continue 'outer;
                }
                cur.push(diffs[i].clone());
                n += 1;
            } else {
                cur.push(diffs[i].clone());
                n = 0;
            }
            i += 1;
        }
    }
    result.into_iter().filter(|(_, l)| !l.is_empty()).collect()
}

/// Get line numbers for hunks
fn find_regions<T: Clone>(hunks: Vec<(bool, Vec<Diff<T>>)>) -> Vec<DiffRegion<T>> {
    let (mut left, mut right) = (1i64, 1i64);
    let mut out = vec![];
    for (output, run) in hunks {
        let (dl, dr) = count_delta(&run);
        if output {
            out.push(DiffRegion { left: (left, dl), right: (right, dr), diffs: run });
        }
        left += dl;
        right += dr;
    }
    out
}

/// Get left/right line counts for a hunk
pub fn count_delta<T>(diffs: &[Diff<T>]) -> (i64, i64) {
    let (mut l, mut r) = (0, 0);
    for d in diffs {
        match d {
            Diff::Both(..) => {
                l += 1;
                r += 1;
            }
            Diff::First(_) => l += 1,
            Diff::Second(_) => r += 1,
        }
    }
    (l, r)
}

fn format_region(color: bool, lf: LfStatus, region: &DiffRegion<String>) -> String {
    let tup = |(a, b): (i64, i64)| format!("{},{}", a, b);
    let header = colorize(color, CYAN, &format!("@@ -{} +{} @@", tup(region.left), tup(region.right)));
    let fmt = |d: &Diff<String>| match d {
        Diff::Both(x, _) => format!(" {}", x),
        Diff::First(x) => colorize(color, RED, &format!("-{}", x)),
        Diff::Second(x) => colorize(color, GREEN, &format!("+{}", x)),
    };
    // getStrings operates on the reversed list; the result is reversed back.
    let rev: Vec<&Diff<String>> = region.diffs.iter().rev().collect();
    let mut strings: Vec<String> = vec![];
    match lf {
        LfStatus::LinefeedOk => strings.extend(rev.iter().map(|d| fmt(d))),
        LfStatus::LinefeedMissing => {
            let mut k = 0;
            while k < rev.len() {
                if matches!(rev[k], Diff::Both(..) | Diff::First(_)) {
                    strings.push("\\ No newline at end of file".to_string());
                    strings.extend(rev[k..].iter().map(|d| fmt(d)));
                    break;
                }
                strings.push(fmt(rev[k]));
                k += 1;
            }
        }
    }
    let mut lines = vec![header];
    lines.extend(strings.into_iter().rev());
    let mut s = String::new();
    for l in lines {
        s.push_str(&l);
        s.push('\n');
    }
    s
}

// git patch does not like `\` on Windows
fn normalize_path(path: &str) -> String {
    path.chars().map(|c| if c == std::path::MAIN_SEPARATOR { '/' } else { c }).collect()
}

fn format_doc(color: bool, doc: &DiffDoc<String>) -> String {
    let a_join = if doc.name.starts_with('/') { doc.name.clone() } else { format!("a/{}", doc.name) };
    let a = normalize_path(&a_join);
    let b_join = if doc.name.starts_with('/') { doc.name.clone() } else { format!("b/{}", doc.name) };
    let b = normalize_path(&b_join);
    let mut s = String::new();
    s.push_str(&colorize(color, BOLD, &format!("--- {}", a)));
    s.push('\n');
    s.push_str(&colorize(color, BOLD, &format!("+++ {}", b)));
    s.push('\n');
    let n = doc.regions.len();
    for (i, r) in doc.regions.iter().enumerate() {
        let lf = if i + 1 == n { doc.lf } else { LfStatus::LinefeedOk };
        s.push_str(&format_region(color, lf, r));
    }
    s
}

/// The fixes `-f diff` would print for a check result: merged with the Fix
/// semigroup and grouped by file name, exactly as the diff formatter does.
/// (Extension: used by the CLI's `--fix`.)
pub fn fixes_by_file(result: &CheckResult) -> BTreeMap<String, Fix> {
    let suggested: Vec<Fix> = result.comments.iter().filter_map(|c| c.fix.clone()).collect();
    build_fix_map(&suggested)
}

/// The contents that `patch -p1` produces when it applies the `-f diff`
/// output for `fix` to `contents`, or `None` if the fix changes nothing.
/// (Extension: used by the CLI's `--fix`.)
///
/// The new lines are those `apply_fix` returns, each ending in a newline.
/// The diff marks a missing final newline only on the last old line it
/// prints, so the result keeps that missing newline only when its last line
/// is an unchanged old line (a `Both` at the end of the diff).
pub fn apply_fix_to_contents(contents: &str, fix: &Fix) -> Option<String> {
    let old = haskell_lines(contents);
    let new = apply_fix(fix, &old);
    if new.len() == old.len() && new.iter().zip(&old).all(|(a, b)| a == b) {
        return None;
    }
    let old: Vec<String> = old.into_iter().map(String::from).collect();
    let diffs = get_diff(&old, &new);
    let mut out = String::with_capacity(contents.len() + 16);
    for line in &new {
        out.push_str(line);
        out.push('\n');
    }
    if !has_trailing_linefeed(contents) && matches!(diffs.last(), Some(Diff::Both(..))) {
        out.pop();
    }
    Some(out)
}

/// Create a Map from filename to Fix
fn build_fix_map(fixes: &[Fix]) -> BTreeMap<String, Fix> {
    let all = merge_fixes(fixes);
    let mut per_file: BTreeMap<String, Fix> = BTreeMap::new();
    for fix in split_fix_by_file(&all) {
        let key = fix.replacements[0].start_pos.file.clone();
        // M.fromListWith mappend: the new value is the left operand.
        let merged = match per_file.remove(&key) {
            Some(old) => merge_fixes(&[fix, old]),
            None => fix,
        };
        per_file.insert(key, merged);
    }
    per_file
}

fn split_fix_by_file(fix: &Fix) -> Vec<Fix> {
    let mut out: Vec<Fix> = vec![];
    let mut last_file: Option<&str> = None;
    for r in &fix.replacements {
        let f = r.start_pos.file.as_str();
        if last_file == Some(f) {
            out.last_mut().unwrap().replacements.push(r.clone());
        } else {
            out.push(Fix { replacements: vec![r.clone()] });
            last_file = Some(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(n: i32) -> Diff<i32> {
        Diff::Both(n, n)
    }
    fn l(n: i32) -> Diff<i32> {
        Diff::First(n)
    }
    fn r(n: i32) -> Diff<i32> {
        Diff::Second(n)
    }

    #[test]
    fn prop_identifies_proper_context() {
        assert_eq!(
            group_diff(&[b(1), b(2), b(3), b(4), l(5), b(6), b(7), b(8), b(9)]),
            vec![(false, vec![b(1)]), (true, vec![b(2), b(3), b(4), l(5), b(6), b(7), b(8)]), (false, vec![b(9)]),]
        );
    }

    #[test]
    fn prop_includes_context_from_start_if_necessary() {
        assert_eq!(
            group_diff(&[b(4), l(5), b(6), b(7), b(8), b(9)]),
            vec![(true, vec![b(4), l(5), b(6), b(7), b(8)]), (false, vec![b(9)])]
        );
    }

    #[test]
    fn prop_includes_context_until_end_if_necessary() {
        assert_eq!(group_diff(&[b(4), l(5)]), vec![(true, vec![b(4), l(5)])]);
    }

    #[test]
    fn prop_splits_into_multiple_hunks() {
        assert_eq!(
            group_diff(&[l(1), b(1), b(2), b(3), b(4), b(5), b(6), b(7), r(8)]),
            vec![(true, vec![l(1), b(1), b(2), b(3)]), (false, vec![b(4)]), (true, vec![b(5), b(6), b(7), r(8)]),]
        );
    }

    #[test]
    fn prop_splits_into_multiple_hunks_unless_touching() {
        assert_eq!(
            group_diff(&[l(1), b(1), b(2), b(3), b(4), b(5), b(6), r(7)]),
            vec![(true, vec![l(1), b(1), b(2), b(3), b(4), b(5), b(6), r(7)])]
        );
    }

    #[test]
    fn prop_count_deltas_works() {
        assert_eq!(count_delta(&[b(1), l(2), r(3), r(4), b(5)]), (3, 4));
    }

    #[test]
    fn prop_count_deltas_works2() {
        assert_eq!(count_delta::<i32>(&[]), (0, 0));
    }

    // Oracle-confirmed ties (koalaman/shellcheck -f diff): a line-LCS that
    // prefers deletions picked a different, equally short script here.
    #[test]
    fn get_diff_breaks_ties_like_data_algorithm_diff() {
        // old: #!, `echo $1 $1`, `echo $1`, `echo "$1"`; every line fixed to `echo "$1"`-like.
        assert_eq!(get_diff(&[0, 1, 2, 3], &[0, 4, 3, 3]), vec![b(0), l(1), l(2), r(4), b(3), r(3)]);
        // `echo $1` then `echo "$1"`, the first one fixed.
        assert_eq!(get_diff(&[0, 1, 2], &[0, 2, 2]), vec![b(0), l(1), b(2), r(2)]);
        assert_eq!(get_diff::<i32>(&[], &[]), vec![]);
        assert_eq!(get_diff(&[1, 2], &[]), vec![l(1), l(2)]);
        assert_eq!(get_diff(&[], &[1, 2]), vec![r(1), r(2)]);
        assert_eq!(get_diff(&[1, 2, 3], &[1, 2, 3]), vec![b(1), b(2), b(3)]);
    }

    fn rep(line: i64, sc: i64, ec: i64, text: &str) -> Fix {
        use shellcheck_core::interface::{InsertionPoint, Position, Replacement};
        let pos = |c| Position { file: "f".into(), line, column: c };
        Fix {
            replacements: vec![Replacement {
                start_pos: pos(sc),
                end_pos: pos(ec),
                string: text.into(),
                precedence: 1,
                insertion_point: InsertionPoint::InsertAfter,
            }],
        }
    }

    #[test]
    fn apply_fix_to_contents_like_patch() {
        // Quote `$1` on line 2.
        let fix = rep(2, 6, 8, "\"$1\"");
        assert_eq!(
            apply_fix_to_contents("#!/bin/sh\necho $1\ntrue\n", &fix).as_deref(),
            Some("#!/bin/sh\necho \"$1\"\ntrue\n")
        );
        // The last line unchanged: its missing newline stays missing.
        assert_eq!(
            apply_fix_to_contents("#!/bin/sh\necho $1\ntrue", &fix).as_deref(),
            Some("#!/bin/sh\necho \"$1\"\ntrue")
        );
        // The last line changed: the `+` line of the diff ends in a newline.
        assert_eq!(apply_fix_to_contents("#!/bin/sh\necho $1", &fix).as_deref(), Some("#!/bin/sh\necho \"$1\"\n"));
        // CRs stay part of the line.
        assert_eq!(
            apply_fix_to_contents("#!/bin/sh\r\necho $1\r\n", &fix).as_deref(),
            Some("#!/bin/sh\r\necho \"$1\"\r\n")
        );
        // A no-op fix changes nothing.
        assert_eq!(apply_fix_to_contents("#!/bin/sh\necho $1\n", &rep(2, 6, 8, "$1")), None);
    }

    #[test]
    fn get_diff_replacement_is_first_then_second() {
        let a = ["x".to_string(), "y".to_string(), "z".to_string()];
        let c = ["x".to_string(), "Y".to_string(), "z".to_string()];
        assert_eq!(
            get_diff(&a, &c),
            vec![
                Diff::Both("x".into(), "x".into()),
                Diff::First("y".into()),
                Diff::Second("Y".into()),
                Diff::Both("z".into(), "z".into())
            ]
        );
    }
}
