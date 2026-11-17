// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression tests for the nesting limit (no
// ShellCheck counterpart). Upstream has no limit and times out or runs out
// of memory on these inputs; the port must stop with SC1072, not abort.
//
// Each input is run through the built `shelllint` binary from stdin. A
// nesting error must exit 1 with SC1072 (and no panic or stack overflow) and
// finish within `TIME_LIMIT`. The boundary tests check the deepest input the
// parser still accepts and the first one it rejects, for each construct.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Generous bound for a debug build; the release build takes milliseconds.
const TIME_LIMIT: Duration = Duration::from_secs(10);
/// The message `MAX_NESTING` produces (`crates/parser/src/engine.rs`).
const NESTING_MESSAGE: &str = "Nesting is deeper than 1000 levels";

struct Run {
    code: Option<i32>,
    json: String,
    stderr: String,
    elapsed: Duration,
}

fn run(script: &str) -> Run {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1", "-"])
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Write on its own thread: the child may exit before reading everything.
    let mut stdin = child.stdin.take().unwrap();
    let input = script.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let o = child.wait_with_output().unwrap();
    writer.join().unwrap();
    Run {
        code: o.status.code(),
        json: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        elapsed: start.elapsed(),
    }
}

fn codes(json: &str) -> Vec<u64> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|e| panic!("bad json ({e}): {json:.300}"));
    v["comments"].as_array().unwrap().iter().map(|c| c["code"].as_u64().unwrap()).collect()
}

/// Asserts that `script` stops with the nesting error: exit 1, SC1072 with
/// the limit message, no panic, and the time bound.
fn assert_rejected(name: &str, script: &str) {
    let r = run(script);
    assert!(r.elapsed < TIME_LIMIT, "{name}: took {:?}", r.elapsed);
    assert!(!r.stderr.contains("panicked") && !r.stderr.contains("overflow"), "{name}: {}", r.stderr);
    assert_eq!(r.code, Some(1), "{name}: exit code\nstderr: {}", r.stderr);
    assert!(codes(&r.json).contains(&1072), "{name}: no SC1072 in {:.300}", r.json);
    assert!(r.json.contains(NESTING_MESSAGE), "{name}: no nesting message in {:.300}", r.json);
}

/// Asserts that `script` is accepted: the parser does not reach the limit
/// (no SC1072), and analysis finishes without a panic or stack overflow.
fn assert_accepted(name: &str, script: &str) {
    let r = run(script);
    assert!(r.elapsed < TIME_LIMIT, "{name}: took {:?}", r.elapsed);
    assert!(!r.stderr.contains("panicked") && !r.stderr.contains("overflow"), "{name}: {}", r.stderr);
    assert!(matches!(r.code, Some(0) | Some(1)), "{name}: exit code {:?}\nstderr: {}", r.code, r.stderr);
    assert!(!codes(&r.json).contains(&1072), "{name}: unexpected SC1072");
}

const N: usize = 100_000;

fn brace_open(n: usize) -> String {
    format!("#!/bin/bash\n{}true;\n", "{ ".repeat(n))
}

fn brace_closed(n: usize) -> String {
    format!("#!/bin/bash\n{}true;{}\n", "{ ".repeat(n), " }".repeat(n))
}

fn dollar_brace_open(n: usize) -> String {
    format!("#!/bin/bash\n{}\n", "${a:-".repeat(n))
}

fn dollar_brace_closed(n: usize) -> String {
    format!("#!/bin/bash\n{}{}\n", "${a:-".repeat(n), "}".repeat(n))
}

fn command_substitution_closed(n: usize) -> String {
    format!("#!/bin/bash\n{}{}\n", "$(".repeat(n), ")".repeat(n))
}

fn quoted_command_substitution_closed(n: usize) -> String {
    format!("#!/bin/bash\n{}{}\n", "\"$(".repeat(n), ")\"".repeat(n))
}

fn arithmetic_open(n: usize) -> String {
    format!("#!/bin/bash\n{}\n", "((".repeat(n))
}

fn arithmetic_closed(n: usize) -> String {
    format!("#!/bin/bash\n{}1{}\n", "((".repeat(n), "))".repeat(n))
}

/// `(( ((...1...)) ))`: one parenthesised group per level inside `((`.
fn arithmetic_group(n: usize) -> String {
    format!("#!/bin/bash\n(({}1{}))\n", "(".repeat(n), ")".repeat(n))
}

#[test]
fn deep_brace_groups_open_are_rejected_at_100k() {
    assert_rejected("brace_open", &brace_open(N));
}

#[test]
fn deep_brace_groups_closed_are_rejected_at_100k() {
    assert_rejected("brace_closed", &brace_closed(N));
}

#[test]
fn deep_parameter_expansion_open_is_rejected_at_100k() {
    assert_rejected("dollar_brace_open", &dollar_brace_open(N));
}

#[test]
fn deep_parameter_expansion_closed_is_rejected_at_100k() {
    assert_rejected("dollar_brace_closed", &dollar_brace_closed(N));
}

#[test]
fn deep_command_substitution_closed_is_rejected_at_100k() {
    assert_rejected("dollar_paren_closed", &command_substitution_closed(N));
}

#[test]
fn deep_quoted_command_substitution_closed_is_rejected_at_100k() {
    assert_rejected("dquote_dollar_paren_closed", &quoted_command_substitution_closed(N));
}

#[test]
fn deep_arithmetic_open_is_rejected_at_100k() {
    assert_rejected("arith_open", &arithmetic_open(N));
}

#[test]
fn deep_arithmetic_closed_is_rejected_at_100k() {
    assert_rejected("arith_closed", &arithmetic_closed(N));
}

#[test]
fn deep_arithmetic_open_is_rejected_at_10k() {
    assert_rejected("arith_open_10k", &arithmetic_open(10_000));
}

// Boundaries, measured on the release build: 998 nested `{` groups (and
// `${`, `((` groups, subshells) are accepted and 999 are rejected; 499 nested
// `$(` are accepted and 500 rejected, since each `$(` also opens a command
// frame. A test fails if the limit moves, which is intended: update these.

#[test]
fn brace_groups_at_limit_are_accepted() {
    assert_accepted("brace_998", &brace_closed(998));
}

#[test]
fn brace_groups_one_past_limit_are_rejected() {
    assert_rejected("brace_999", &brace_closed(999));
    assert_rejected("brace_1000", &brace_closed(1000));
}

#[test]
fn parameter_expansion_at_limit_is_accepted() {
    assert_accepted("dollar_brace_998", &dollar_brace_closed(998));
}

#[test]
fn parameter_expansion_one_past_limit_is_rejected() {
    assert_rejected("dollar_brace_999", &dollar_brace_closed(999));
}

#[test]
fn command_substitution_at_limit_is_accepted() {
    assert_accepted("dollar_paren_499", &command_substitution_closed(499));
}

#[test]
fn command_substitution_one_past_limit_is_rejected() {
    assert_rejected("dollar_paren_500", &command_substitution_closed(500));
}

#[test]
fn arithmetic_groups_at_limit_are_accepted() {
    assert_accepted("arith_group_998", &arithmetic_group(998));
}

#[test]
fn arithmetic_groups_one_past_limit_are_rejected() {
    assert_rejected("arith_group_999", &arithmetic_group(999));
}
