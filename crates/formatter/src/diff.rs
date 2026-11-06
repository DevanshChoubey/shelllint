// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Formatter/Diff.hs (e11f964)
//
// DIVERGENCE: `Data.Algorithm.Diff.getDiff` is replaced by a line-based LCS
// that takes a common-prefix-first greedy diagonal and prefers deletions
// before insertions. This yields the same script as Haskell for all cases
// where the minimal edit script is unique, and for replaced lines (`-` then
// `+`); ties between equally short scripts over repeated lines may differ.
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
    DiffFormatter {
        found_issues: false,
        reported_issues: false,
        color: should_output_color(options.color_option),
    }
}

fn ansi(n: i32) -> String {
    format!("\x1B[{}m", n)
}

fn colorize(on: bool, n: i32, s: &str) -> String {
    if on {
        format!("{}{}{}", ansi(n), s, ansi(0))
    } else {
        s.to_string()
    }
}

fn print_err(color: bool, msg: &str) {
    eprintln!("{}", colorize(color, BOLD, &colorize(color, RED, msg)));
}

impl Formatter for DiffFormatter {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn footer(&mut self, _out: &mut dyn Write) {
        if self.found_issues && !self.reported_issues {
            print_err(
                self.color,
                "Issues were detected, but none were auto-fixable. Use another format to see them.",
            );
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
        let suggested: Vec<Fix> = comments.iter().filter_map(|c| c.fix.clone()).collect();
        let fixmap = build_fix_map(&suggested);
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

/// Stand-in for Data.Algorithm.Diff.getDiff (see DIVERGENCE above).
pub fn get_diff<T: Clone + PartialEq>(a: &[T], b: &[T]) -> Vec<Diff<T>> {
    let (n, m) = (a.len(), b.len());
    // Strip common prefix and suffix, then LCS table on the middle.
    let mut pre = 0;
    while pre < n && pre < m && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < n - pre && suf < m - pre && a[n - 1 - suf] == b[m - 1 - suf] {
        suf += 1;
    }
    let (am, bm) = (&a[pre..n - suf], &b[pre..m - suf]);
    let (p, q) = (am.len(), bm.len());
    let w = q + 1;
    let mut t = vec![0u32; (p + 1) * w];
    for i in (0..p).rev() {
        for j in (0..q).rev() {
            t[i * w + j] = if am[i] == bm[j] {
                t[(i + 1) * w + j + 1] + 1
            } else {
                t[(i + 1) * w + j].max(t[i * w + j + 1])
            };
        }
    }
    let mut out: Vec<Diff<T>> = a[..pre].iter().map(|x| Diff::Both(x.clone(), x.clone())).collect();
    let (mut i, mut j) = (0, 0);
    while i < p || j < q {
        if i < p && j < q && am[i] == bm[j] {
            out.push(Diff::Both(am[i].clone(), bm[j].clone()));
            i += 1;
            j += 1;
        } else if i < p && (j == q || t[(i + 1) * w + j] >= t[i * w + j + 1]) {
            out.push(Diff::First(am[i].clone()));
            i += 1;
        } else {
            out.push(Diff::Second(bm[j].clone()));
            j += 1;
        }
    }
    out.extend(a[n - suf..].iter().map(|x| Diff::Both(x.clone(), x.clone())));
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
            vec![
                (false, vec![b(1)]),
                (true, vec![b(2), b(3), b(4), l(5), b(6), b(7), b(8)]),
                (false, vec![b(9)]),
            ]
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
            vec![
                (true, vec![l(1), b(1), b(2), b(3)]),
                (false, vec![b(4)]),
                (true, vec![b(5), b(6), b(7), r(8)]),
            ]
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
