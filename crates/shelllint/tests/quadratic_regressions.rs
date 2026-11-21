// SPDX-License-Identifier: GPL-3.0-or-later
// Timing regression tests for the public API on long lines and many fixes.
// No Haskell counterpart.
//
// Each test runs the full linter (parse, checks, and the tab-stop realignment
// `check_str` does for every comment) or the fix path, and requires it to finish
// within `BOUND` in the debug build. The quadratic versions took tens of seconds
// at these sizes, so the bound leaves a wide margin.

use std::time::{Duration, Instant};

use shelllint::{Linter, apply_fixes};

const BOUND: Duration = Duration::from_secs(5);

/// 20000 unquoted `$a` on one line, `a` unknown to the flow analysis, so SC2086
/// fires once per word. Each comment used to copy the whole line to realign it.
#[test]
fn twenty_thousand_warnings_on_one_long_line_check_in_time() {
    let script = "#!/bin/bash\na=$1; echo".to_string() + &" $a".repeat(20_000) + "\n";
    let start = Instant::now();
    let diags = Linter::new().check_str(&script);
    let elapsed = start.elapsed();
    assert!(elapsed < BOUND, "check_str took {elapsed:?}");
    assert_eq!(diags.iter().filter(|d| d.code == 2086).count(), 20_000);
}

/// The same line with `--fix`: 10000 quoting fixes applied in one pass. Applying
/// them used to take quadratic time in the number of fixes.
#[test]
fn ten_thousand_fixes_on_one_long_line_apply_in_time() {
    let script = "#!/bin/bash\na=$1; echo".to_string() + &" $a".repeat(10_000) + "\n";
    let diags = Linter::new().check_str(&script);
    let start = Instant::now();
    let fixed = apply_fixes(&script, &diags);
    let elapsed = start.elapsed();
    assert!(elapsed < BOUND, "apply_fixes took {elapsed:?}");
    let expected = "#!/bin/bash\na=$1; echo".to_string() + &" \"$a\"".repeat(10_000) + "\n";
    assert_eq!(fixed, expected);
}
