// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression tests for parser run time (no
// ShellCheck counterpart). Each case was slow or memory-hungry before the fix
// in the parser. The bounds are generous for a debug build; the release build
// takes a fraction of a second on each input.
#![allow(dead_code)]

#[path = "zsh_common.rs"]
mod zsh_common;
use std::time::{Duration, Instant};
use zsh_common::*;

const LIMIT: Duration = Duration::from_secs(20);

fn timed_zsh_codes(script: &str) -> (Vec<u32>, Duration) {
    let start = Instant::now();
    let codes = zsh_codes(script);
    (codes, start.elapsed())
}

/// zsh, F6: `read_function_definition` used to rescan the rest of the line for
/// `()` on every compound attempt, so one long line of simple commands cost
/// O(commands x line length). A long line with no `()` must now take about
/// linear time.
#[test]
fn zsh_long_line_without_parens_finishes_fast() {
    let script = format!("{}\n", "echo a; ".repeat(40_000));
    let (codes, took) = timed_zsh_codes(&script);
    assert!(took < LIMIT, "40k commands on one zsh line took {took:?}");
    assert!(!codes.contains(&1072), "a plain long line must parse: {codes:?}");
}

/// zsh, F6: the `()` test must still see a function definition at the end of
/// a long line. The line must parse without the hard parse codes.
#[test]
fn zsh_long_line_with_function_at_end_still_parses() {
    let script = format!("{}f() {{ echo; }}\n", "echo a; ".repeat(20_000));
    let (codes, took) = timed_zsh_codes(&script);
    assert!(took < LIMIT, "long line with a function took {took:?}");
    assert!(!codes.iter().any(|c| HARD_PARSE_CODES.contains(c)), "function on a long line rejected: {codes:?}");
}

/// zsh, F8: `zsh_closes_group` walked back over the whole word for every `}`.
/// The fix removes that walk, but a word of many `}` still costs more than
/// linear time in shared code (it is slow under `--shell=bash` too), so this
/// bound only guards against a large regression at 4k braces.
#[test]
fn zsh_word_of_many_braces_finishes_fast() {
    let script = format!("echo {}\n", "a}".repeat(4_000));
    let (_, took) = timed_zsh_codes(&script);
    assert!(took < LIMIT, "4k braces in one zsh word took {took:?}");
}

