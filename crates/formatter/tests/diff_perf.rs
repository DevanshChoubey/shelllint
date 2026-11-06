// SPDX-License-Identifier: GPL-3.0-only
// Resource bound for the diff formatter's `getDiff` (Data.Algorithm.Diff,
// O((N+M)D)); no Haskell counterpart.
//
// A 20000-line file with a fix on every 10th line (D = 4000) used to need a
// full LCS table, 20000^2 * 4 bytes = 1.6 GB. The test re-runs itself in a
// child process under `ulimit -v`, so a regression fails instead of
// exhausting the machine's memory, and checks the child's time and VmHWM.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use shellcheck_formatter::diff::{Diff, get_diff};

const CHILD_ENV: &str = "SC_DIFF_PERF_CHILD";
const TIME_LIMIT: Duration = Duration::from_secs(30);
const RSS_LIMIT_MB: u64 = 200;
const AS_LIMIT_KB: u64 = 1_500_000;

fn peak_rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

fn work() {
    let old: Vec<String> = (0..20000).map(|i| format!("echo $v{i}")).collect();
    let new: Vec<String> =
        (0..20000).map(|i| if i % 10 == 0 { format!("echo \"$v{i}\"") } else { format!("echo $v{i}") }).collect();
    let d = get_diff(&old, &new);
    assert_eq!(d.iter().filter(|x| matches!(x, Diff::Both(..))).count(), 18000);
    assert_eq!(d.len(), 22000);
}

#[test]
fn get_diff_is_not_quadratic_in_memory() {
    if std::env::var_os(CHILD_ENV).is_some() {
        work();
        println!("PEAK_RSS_MB={}", peak_rss_mb());
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let start = Instant::now();
    let out = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("ulimit -v {AS_LIMIT_KB} && exec timeout {} \"$0\" \"$@\"", TIME_LIMIT.as_secs() + 5))
        .arg(&exe)
        .args(["--exact", "get_diff_is_not_quadratic_in_memory", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "child failed: {stdout}{}", String::from_utf8_lossy(&out.stderr));
    let rss: u64 = stdout
        .split("PEAK_RSS_MB=")
        .nth(1)
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .unwrap_or_else(|| panic!("child printed no PEAK_RSS_MB: {stdout}"));
    eprintln!("get_diff: {elapsed:?}, peak RSS {rss} MB");
    assert!(elapsed < TIME_LIMIT, "took {elapsed:?}");
    assert!(rss < RSS_LIMIT_MB, "peak RSS {rss} MB");
}
