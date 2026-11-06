// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Prelude.hs (e11f964)

// Generic basic utility functions

/// Get element 0 or a default. Like `head` but safe.
pub fn head_or_default<T: Clone>(def: T, list: &[T]) -> T {
    list.first().cloned().unwrap_or(def)
}

/// Get the last element or a default. Like `last` but safe.
pub fn last_or_default<T: Clone>(def: T, list: &[T]) -> T {
    list.last().cloned().unwrap_or(def)
}

/// `(!!!)`: get element n of a list, or Nothing. Like `!!` but safe.
pub fn nth<T>(list: &[T], i: usize) -> Option<&T> {
    list.get(i)
}

// DIVERGENCE: not in Prelude.hs; `fromMaybe def (list !!! i)` as one call.
pub fn nth_or_default<T: Clone>(def: T, list: &[T], i: usize) -> T {
    nth(list, i).cloned().unwrap_or(def)
}

/// Like mconcat but for Semigroups. `x <> sconcat1 xs`, so right-associative.
// DIVERGENCE: the Semigroup `<>` is passed explicitly as `op`.
pub fn sconcat1<T>(list: Vec<T>, op: impl Fn(T, T) -> T) -> T {
    list.into_iter()
        .rev()
        .reduce(|acc, x| op(x, acc))
        .unwrap_or_else(|| panic!("{}", please_report("sconcat1 of empty list")))
}

pub fn sconcat_or_default<T>(def: T, list: Vec<T>, op: impl Fn(T, T) -> T) -> T {
    if list.is_empty() { def } else { sconcat1(list, op) }
}

/// For more actionable "impossible" errors
pub fn please_report(str: &str) -> String {
    format!("ShellCheck internal error, please report: {str}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(head_or_default(0, &[3, 4]), 3);
        assert_eq!(head_or_default(0, &[]), 0);
        assert_eq!(last_or_default(0, &[3, 4]), 4);
        assert_eq!(last_or_default(0, &[]), 0);
        assert_eq!(nth(&[3, 4], 1), Some(&4));
        assert_eq!(nth(&[3, 4], 2), None);
        assert_eq!(nth_or_default(9, &[3, 4], 2), 9);
    }

    #[test]
    fn sconcat_is_right_associative() {
        let parts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(sconcat1(parts, |x, y| format!("({x}{y})")), "(a(bc))");
        assert_eq!(sconcat_or_default(7, vec![], |x: i32, y| x + y), 7);
    }

    #[test]
    fn please_report_message() {
        assert_eq!(please_report("x"), "ShellCheck internal error, please report: x");
    }
}
