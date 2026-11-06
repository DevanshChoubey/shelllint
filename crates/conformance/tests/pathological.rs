// SPDX-License-Identifier: GPL-3.0-only
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart
//
// Pathological-input tests: each input must go through the full checker
// pipeline in under 10 s with a peak RSS under 512 MB.
//
// VmHWM is process-wide and tests run concurrently, so each test re-runs
// itself in a child process (this test binary, `--exact <name>`, with
// `SC_PATHOLOGICAL_CHILD` set). The child runs the pipeline and prints its
// own VmHWM; the parent waits for it on a worker thread with
// `recv_timeout`, killing it after the deadline.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "SC_PATHOLOGICAL_CHILD";
const TIME_LIMIT: Duration = Duration::from_secs(10);
const RSS_LIMIT_MB: u64 = 512;

/// Peak resident set size in MB (VmHWM), Linux only.
fn peak_rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

fn run_pipeline(script: &str) {
    #[cfg(feature = "checker")]
    {
        // check_script runs on its own big-stack thread.
        let _ = shellcheck_checker::check_script_codes(script, None);
    }
    #[cfg(not(feature = "checker"))]
    {
        let _ = script;
        panic!("unported: checker feature disabled");
    }
}

fn check(name: &str, script: impl FnOnce() -> String) {
    if std::env::var_os(CHILD_ENV).is_some() {
        let script = script();
        run_pipeline(&script);
        println!("PEAK_RSS_MB={}", peak_rss_mb());
        return;
    }
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    // stderr is drained on its own thread so a full stderr pipe cannot
    // block the child while stdout is being read.
    let err_reader = std::thread::spawn(move || {
        let mut err = String::new();
        let _ = stderr.read_to_string(&mut err);
        err
    });
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = String::new();
        let _ = stdout.read_to_string(&mut out);
        let err = err_reader.join().unwrap_or_default();
        let _ = tx.send((out, err));
    });
    let (out, err) = match rx.recv_timeout(TIME_LIMIT) {
        Ok(o) => o,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{name}: took longer than {TIME_LIMIT:?}");
        }
    };
    let status = child.wait().unwrap();
    let elapsed = start.elapsed();
    assert!(status.success(), "{name}: child failed ({status}):\n{out}\n{err}");
    let rss: u64 = out
        .lines()
        .find_map(|l| l.split_once("PEAK_RSS_MB=").map(|(_, v)| v))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("{name}: no PEAK_RSS_MB in child output:\n{out}\n{err}"));
    eprintln!("{name}: {elapsed:?}, peak RSS {rss} MB");
    assert!(elapsed < TIME_LIMIT, "{name}: took {elapsed:?}");
    assert!(rss < RSS_LIMIT_MB, "{name}: peak RSS {rss} MB");
}

#[test]
fn deeply_nested_command_substitution() {
    check("deeply_nested_command_substitution", || "$(".repeat(5000) + &")".repeat(5000));
}

#[test]
fn fifty_thousand_lines() {
    check("fifty_thousand_lines", || "echo $x\n".repeat(50_000));
}

#[test]
fn one_megabyte_line() {
    check("one_megabyte_line", || "a".repeat(1024 * 1024));
}
