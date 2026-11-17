// SPDX-License-Identifier: GPL-3.0-or-later
// Timing regression tests for checks that used to cost quadratic time; no
// Haskell counterpart. Long-line and fix-path tests live in crates/shelllint.
//
// Each test runs one registered check in this process on a generated script
// and requires it, parse included, to finish within `BOUND` in the debug
// build. The bound is generous: the release build takes a small fraction of it,
// while the quadratic versions took tens of seconds at these sizes.

use std::time::{Duration, Instant};

use shelllint_analyzer::testing::run_check;
use shelllint_checks::CHECKS;

const BOUND: Duration = Duration::from_secs(5);

/// Run `module`:`name` (a `node` check) on `script`, returning its codes in
/// order, and fail if that takes longer than `BOUND`.
fn timed_codes(module: &str, name: &str, script: &str) -> Vec<i64> {
    let start = Instant::now();
    let codes = run_check(CHECKS, module, name, "node", script).expect("check is registered");
    let elapsed = start.elapsed();
    assert!(elapsed < BOUND, "{module}:{name} took {elapsed:?} on {} bytes", script.len());
    codes.into_iter().collect()
}

/// 8000 functions after a top-level `exit`, so every function is never
/// invoked. Each used to walk the whole tree for `trap` actions, and under
/// zsh also for the registered-function names.
#[test]
fn eight_thousand_unreachable_functions_bash() {
    let script = unreachable_functions("#!/bin/bash\n", 8000);
    let codes = timed_codes("Analytics.hs", "checkCommandIsUnreachable", &script);
    // One SC2317 for the body of each function and one SC2329 for the function.
    assert_eq!(codes.len(), 16_000);
    assert!(codes.iter().all(|&c| c == 2317 || c == 2329), "unexpected codes");
}

#[test]
fn eight_thousand_unreachable_functions_zsh() {
    let script = unreachable_functions("#!/bin/zsh\n", 8000);
    let codes = timed_codes("Analytics.hs", "checkCommandIsUnreachable", &script);
    assert_eq!(codes.len(), 16_000);
    assert!(codes.iter().all(|&c| c == 2317 || c == 2329), "unexpected codes");
}

fn unreachable_functions(shebang: &str, n: usize) -> String {
    let mut s = String::from(shebang) + "echo start\nexit 0\n";
    for i in 0..n {
        s += &format!("f{i}() {{ echo {i}; }}\n");
    }
    s
}
