// SPDX-License-Identifier: GPL-3.0-only
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart
//
// Pathological-input tests. Ignored until Task 15 (full checker pipeline)
// exists; Task 15 enables them and wires `run_pipeline` to the checker.

use std::time::{Duration, Instant};

/// Peak resident set size in MB (VmHWM), Linux only.
fn peak_rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

// DIVERGENCE: the pipeline runs on a thread with a 64 MB stack because the
// recursive-descent parser recurses once per nesting level (5000 nested `$(`).
fn run_pipeline(script: String) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            #[cfg(feature = "checker")]
            {
                let _ = shellcheck_checker::check_script_codes(&script, None);
            }
            #[cfg(not(feature = "checker"))]
            {
                let _ = script;
                panic!("unported: checker feature disabled");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

fn check(script: String) {
    let start = Instant::now();
    run_pipeline(script);
    assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
    assert!(peak_rss_mb() < 512, "peak RSS {} MB", peak_rss_mb());
}

#[test]
#[ignore = "enabled by Task 15"]
fn deeply_nested_command_substitution() {
    check("$(".repeat(5000) + &")".repeat(5000));
}

#[test]
#[ignore = "enabled by Task 15"]
fn fifty_thousand_lines() {
    check("echo $x\n".repeat(50_000));
}

#[test]
#[ignore = "enabled by Task 15"]
fn one_megabyte_line() {
    check("a".repeat(1024 * 1024));
}
