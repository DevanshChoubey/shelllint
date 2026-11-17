// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression tests for the zsh parser's run time
// (no ShellCheck counterpart). Each input made the parser take exponential
// time before the fix; the tests check that it now finishes and that the
// diagnostics for small inputs did not change.
#![allow(dead_code)]

#[path = "zsh_common.rs"]
mod zsh_common;
use std::time::{Duration, Instant};
use zsh_common::*;

/// Generous for a debug build; the release build takes a few milliseconds.
const LIMIT: Duration = Duration::from_secs(5);

fn timed_zsh_codes(script: &str) -> (Vec<u32>, Duration) {
    let start = Instant::now();
    let codes = zsh_codes(script);
    (codes, start.elapsed())
}

/// `echo ${a:-` followed by `k` unclosed `{` and nothing else.
fn unclosed_braces(k: usize) -> String {
    format!("echo ${{a:-{}\n", "{".repeat(k))
}

#[test]
fn unclosed_braces_in_dollar_braces_finish_fast_at_sixty() {
    let (codes, took) = timed_zsh_codes(&unclosed_braces(60));
    assert!(took < LIMIT, "60 unclosed braces took {took:?}");
    assert!(!codes.is_empty(), "expected parse diagnostics for unclosed braces");
}

#[test]
fn unclosed_braces_in_dollar_braces_finish_fast_at_two_hundred() {
    let (codes, took) = timed_zsh_codes(&unclosed_braces(200));
    assert!(took < LIMIT, "200 unclosed braces took {took:?}");
    assert!(!codes.is_empty());
}

#[test]
fn unclosed_braces_in_dollar_braces_same_diagnostics_as_small_input() {
    // Same codes at every size: the parse failure is reported the same way.
    let small = zsh_codes(&unclosed_braces(4));
    assert_eq!(small, vec![1009, 1073, 1072], "small case codes changed");
    for k in [8, 60] {
        assert_eq!(zsh_codes(&unclosed_braces(k)), small, "codes differ at k={k}");
    }
}

#[test]
fn unclosed_braces_with_quotes_inside_finish_fast() {
    // Balanced-looking input whose nested attempts fail further in.
    let script = format!("echo ${{a:-{}\n", "{x '}".repeat(60));
    let (_, took) = timed_zsh_codes(&script);
    assert!(took < LIMIT, "quoted unclosed braces took {took:?}");
}
