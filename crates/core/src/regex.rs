// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Regex.hs (e11f964)
//
// Basically Text.Regex based on regex-tdfa instead of the buggy regex-posix.
//
// DIVERGENCE: Haskell uses regex-tdfa (POSIX ERE, leftmost-longest); this uses
// the `regex` crate (leftmost-first). Alternations whose earlier branch is a
// prefix of a later one (`a|ab`) can pick a shorter match. Other known
// differences: `\` inside a bracket expression is an escape here but literal
// in POSIX; an unmatched `{` is an error here but literal in tdfa; and with
// tdfa's default `multiline = True` a negated class `[^...]` does not match
// newline, while here it does. ShellCheck's own patterns are written to avoid
// these cases; callers porting a pattern must check it against this list.

use regex::{Regex, RegexBuilder};

/// Precompile the regex
// tdfa's defaultCompOpt has `multiline = True`: `^`/`$` match at line
// boundaries and `.` does not match newline (the `regex` default).
pub fn mk_regex(str: &str) -> Regex {
    RegexBuilder::new(str)
        .multi_line(true)
        .build()
        .unwrap_or_else(|e| panic!("Invalid regex {str:?}: {e}"))
}

/// Does the regex match?
pub fn matches(str: &str, re: &Regex) -> bool {
    re.is_match(str)
}

/// Get all subgroups of the first match
pub fn match_regex(re: &Regex, str: &str) -> Option<Vec<String>> {
    re.captures(str).map(|c| groups(&c))
}

// tdfa reports an unmatched group as "".
fn groups(c: &regex::Captures) -> Vec<String> {
    c.iter()
        .skip(1)
        .map(|m| m.map_or(String::new(), |m| m.as_str().to_string()))
        .collect()
}

/// Get all full matches
// Like Haskell, each step re-matches on the remaining suffix (so `^` can
// match at the start of every suffix).
// DIVERGENCE: Haskell loops forever after an empty match; we stop after it.
pub fn match_all_strings(re: &Regex, str: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = str;
    while let Some(m) = re.find(rest) {
        out.push(m.as_str().to_string());
        if m.as_str().is_empty() {
            break;
        }
        rest = &rest[m.end()..];
    }
    out
}

/// Get all subgroups from all matches (`matchAllSubgroups`)
// DIVERGENCE: Haskell loops forever after an empty match; we stop after it.
pub fn match_all_submatches(re: &Regex, str: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rest = str;
    while let Some(c) = re.captures(rest) {
        let m = c.get(0).expect("group 0 always participates");
        out.push(groups(&c));
        if m.as_str().is_empty() {
            break;
        }
        rest = &rest[m.end()..];
    }
    out
}

/// Replace regex in input with string
pub fn sub_regex(re: &Regex, input: &str, replacement: &str) -> String {
    let mut out = String::new();
    let mut str = input;
    while let Some(m) = re.find(str) {
        if m.as_str().is_empty() {
            panic!("Internal error: substituted empty in {str}");
        }
        out.push_str(&str[..m.start()]);
        out.push_str(replacement);
        str = &str[m.end()..];
    }
    out.push_str(str);
    out
}

/// Split a string based on a regex.
// DIVERGENCE: Haskell loops forever on an empty match; we stop splitting.
pub fn split_on(input: &str, re: &Regex) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = input;
    while let Some(m) = re.find(rest) {
        if m.as_str().is_empty() {
            break;
        }
        out.push(rest[..m.start()].to_string());
        rest = &rest[m.end()..];
    }
    out.push(rest.to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_regex_groups() {
        let re = mk_regex("([a-z]+)=([0-9]+)?");
        assert_eq!(match_regex(&re, "x foo=12"), Some(vec!["foo".into(), "12".into()]));
        assert_eq!(match_regex(&re, "foo="), Some(vec!["foo".into(), "".into()]));
        assert_eq!(match_regex(&re, "123"), None);
    }

    #[test]
    fn match_all() {
        let re = mk_regex("[0-9]+");
        assert_eq!(match_all_strings(&re, "a1b22c333"), vec!["1", "22", "333"]);
        let re = mk_regex("([a-z])([0-9])");
        assert_eq!(
            match_all_submatches(&re, "a1 b2"),
            vec![vec!["a".to_string(), "1".into()], vec!["b".into(), "2".into()]]
        );
    }

    #[test]
    fn anchors_rematch_each_suffix() {
        // Haskell re-runs the regex on the remaining string, so `^` matches
        // again after every match.
        assert_eq!(match_all_strings(&mk_regex("^a"), "aab"), vec!["a", "a"]);
    }

    #[test]
    fn sub_and_split() {
        let re = mk_regex("[[:space:]]+");
        assert_eq!(sub_regex(&re, "a  b c", "_"), "a_b_c");
        assert_eq!(split_on("a  b c", &re), vec!["a", "b", "c"]);
        assert_eq!(split_on("abc", &re), vec!["abc"]);
    }

    #[test]
    fn multiline_anchors() {
        assert!(matches("x\nfoo", &mk_regex("^foo$")));
        assert!(!matches("a\nb", &mk_regex("a.b")));
    }
}
