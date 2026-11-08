// SPDX-License-Identifier: GPL-3.0-or-later
// Conformance harness for the port of ShellCheck (e11f964); no Haskell counterpart
//
// Pathological-input tests: each input must go through the full checker
// pipeline in under 10 s with a peak RSS under 512 MB (150 MB for the wide
// control-flow inputs, whose CFG dataflow memory used to grow quadratically).
//
// VmHWM is process-wide and tests run concurrently, so each test re-runs
// itself in a child process (this test binary, `--exact <name>`, with
// `SC_PATHOLOGICAL_CHILD` set). The child runs the pipeline and prints its
// own VmHWM; the parent waits for it on a worker thread with
// `recv_timeout`, killing it after the deadline. The wide control-flow
// tests also cap the child's address space (`ulimit -v` via /bin/sh), so a
// regression fails the test instead of exhausting the machine's memory.

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
        let _ = shelllint_checker::check_script_codes(script, None);
    }
    #[cfg(not(feature = "checker"))]
    {
        let _ = script;
        panic!("unported: checker feature disabled");
    }
}

fn check(name: &str, script: impl FnOnce() -> String) {
    check_with(name, RSS_LIMIT_MB, None, script)
}

/// `check` with a peak-RSS limit and, optionally, an address-space limit
/// (in KB) for the child.
fn check_with(name: &str, rss_limit_mb: u64, as_limit_kb: Option<u64>, script: impl FnOnce() -> String) {
    if std::env::var_os(CHILD_ENV).is_some() {
        let script = script();
        run_pipeline(&script);
        println!("PEAK_RSS_MB={}", peak_rss_mb());
        return;
    }
    let start = Instant::now();
    let exe = std::env::current_exe().unwrap();
    let args = ["--exact", name, "--nocapture", "--test-threads=1"];
    let mut command = match as_limit_kb {
        None => {
            let mut c = Command::new(&exe);
            c.args(args);
            c
        }
        Some(kb) => {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(format!("ulimit -v {kb} && exec \"$0\" \"$@\"")).arg(&exe).args(args);
            c
        }
    };
    let mut child = command.env(CHILD_ENV, "1").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
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
    assert!(rss < rss_limit_mb, "{name}: peak RSS {rss} MB (limit {rss_limit_mb} MB)");
}

/// Peak RSS limit for the wide control-flow inputs.
const CFG_RSS_LIMIT_MB: u64 = 150;
/// Address-space cap for their child: the 512 MB parser stack plus heap.
const CFG_AS_LIMIT_KB: u64 = 2 * 1024 * 1024;

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

/// A case statement with 800 branches, each assigning its own variable.
#[test]
fn case_with_800_branches() {
    check_with("case_with_800_branches", CFG_RSS_LIMIT_MB, Some(CFG_AS_LIMIT_KB), || {
        let mut s = String::from("#!/bin/bash\ncase $1 in\n");
        for i in 0..800 {
            s += &format!("  a{i}) v{i}=$((i+1)); x=\"$x $v{i}\";;\n");
        }
        s + "esac\necho $x\n"
    });
}

/// 400 sequential ifs, each assigning its own variable.
#[test]
fn four_hundred_sequential_ifs() {
    check_with("four_hundred_sequential_ifs", CFG_RSS_LIMIT_MB, Some(CFG_AS_LIMIT_KB), || {
        let mut s = String::from("#!/bin/bash\n");
        for i in 0..400 {
            s += &format!("if [ \"$a\" = {i} ]; then v{i}=1; fi\n");
        }
        s
    });
}
