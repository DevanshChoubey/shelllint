// SPDX-License-Identifier: GPL-3.0-or-later
// Pathological-input tests for single checks of the port of ShellCheck
// (e11f964); no Haskell counterpart.
//
// Each test runs one registered check (`testing::run_check`, which parses
// the script and builds the analysis parameters first) on a generated input
// that used to take quadratic time or memory, and requires it to finish
// within `TIME_LIMIT` with a peak RSS under `RSS_LIMIT_MB`.
//
// As in crates/conformance/tests/pathological.rs, each test re-runs itself
// in a child process (this test binary, `--exact <name>`, with
// `SC_CHECK_PATHOLOGICAL_CHILD` set) whose address space is capped with
// `ulimit -v`, so a regression fails the test instead of exhausting the
// machine's memory. The child prints its own VmHWM; the parent kills it
// after the deadline.

// Uses /bin/sh with `ulimit -v` (and /proc), which only Linux supports.
#![cfg(target_os = "linux")]

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use shellcheck_analyzer::testing::run_check;
use shellcheck_checks::CHECKS;

const CHILD_ENV: &str = "SC_CHECK_PATHOLOGICAL_CHILD";
const TIME_LIMIT: Duration = Duration::from_secs(10);
const RSS_LIMIT_MB: u64 = 200;
/// Address-space cap for the child: the 256 MB check thread stack plus heap.
const AS_LIMIT_KB: u64 = 2 * 1024 * 1024;

/// Peak resident set size in MB (VmHWM), Linux only.
fn peak_rss_mb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    s.lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

/// Run check `module`:`name` (corpus kind `kind`) on `script()` in a capped
/// child process and bound its time and peak RSS. `expect` is checked
/// against the sorted codes in the child.
fn check(
    test: &str,
    (module, name, kind): (&'static str, &'static str, &'static str),
    script: impl FnOnce() -> String,
    expect: impl FnOnce(&[i64]) + Send + 'static,
) {
    if std::env::var_os(CHILD_ENV).is_some() {
        let script = script();
        // The parser recurses; give it a big stack, as the CLI does.
        let codes = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(move || run_check(CHECKS, module, name, kind, &script).expect("check is registered"))
            .unwrap()
            .join()
            .unwrap();
        let codes: Vec<i64> = codes.into_iter().collect();
        expect(&codes);
        println!("PEAK_RSS_MB={}", peak_rss_mb());
        return;
    }
    let start = Instant::now();
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("ulimit -v {AS_LIMIT_KB} && exec \"$0\" \"$@\""))
        .arg(&exe)
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
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
            panic!("{test}: took longer than {TIME_LIMIT:?}");
        }
    };
    let status = child.wait().unwrap();
    let elapsed = start.elapsed();
    assert!(status.success(), "{test}: child failed ({status}):\n{out}\n{err}");
    let rss: u64 = out
        .lines()
        .find_map(|l| l.split_once("PEAK_RSS_MB=").map(|(_, v)| v))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("{test}: no PEAK_RSS_MB in child output:\n{out}\n{err}"));
    eprintln!("{test}: {elapsed:?}, peak RSS {rss} MB");
    assert!(elapsed < TIME_LIMIT, "{test}: took {elapsed:?}");
    assert!(rss < RSS_LIMIT_MB, "{test}: peak RSS {rss} MB (limit {RSS_LIMIT_MB} MB)");
}

/// `find .` with 600 `-exec sh -c "echo {}" \;`: the backtracking match
/// used to emit one comment per (-exec, sh, -c) triple, ~36M of them.
#[test]
fn find_with_600_exec_sh() {
    check(
        "find_with_600_exec_sh",
        ("Checks/Commands.hs", "checkInjectableFindSh", "node"),
        || {
            let mut s = String::from("#!/bin/bash\nfind .");
            for _ in 0..600 {
                s += " -exec sh -c \"echo {}\" \\;";
            }
            s + "\n"
        },
        // Every `"echo {}"` after the first `-c` is reached.
        |codes| assert_eq!(codes.len(), 600),
    );
}

/// 8000 `rm -rf` commands: each used to clone the whole arena.
#[test]
fn eight_thousand_recursive_rms() {
    check(
        "eight_thousand_recursive_rms",
        ("Checks/Commands.hs", "checkCatastrophicRm", "node"),
        || {
            let mut s = String::from("#!/bin/bash\n");
            for i in 0..8000 {
                s += &format!("rm -rf \"$d/x{i}\"\n");
            }
            s + "rm -rf \"$d\"/{,usr}\n"
        },
        // Only the last line: `"$d"/` and `"$d"/usr` may become `/` and `/usr`.
        |codes| assert_eq!(codes, [2115, 2115]),
    );
}

/// `set -e` and 40000 top-level commands: `dropLast` used to copy the
/// list quadratically and recurse once per element.
#[test]
fn forty_thousand_commands_with_errexit() {
    check(
        "forty_thousand_commands_with_errexit",
        ("Analytics.hs", "checkUselessBang", "node"),
        || "#!/bin/bash\nset -e\n".to_string() + &"true\n".repeat(40_000) + "! true\ntrue\n",
        |codes| assert!(!codes.is_empty() && codes.iter().all(|&c| c == 2251), "{codes:?}"),
    );
}

/// 3000 unassigned references against 3000 assigned names: each
/// reference used to run a full Levenshtein against every name.
#[test]
fn three_thousand_misspelling_candidates() {
    check(
        "three_thousand_misspelling_candidates",
        ("Analytics.hs", "checkUnassignedReferences", "tree"),
        || {
            let mut s = String::from("#!/bin/bash\n");
            for i in 0..3000 {
                s += &format!("variable_number_{i}=1\n");
            }
            s += "echo";
            for i in 0..3000 {
                s += &format!(" \"$ref_value_{i}\"");
            }
            s + "\n"
        },
        |codes| assert_eq!(codes.len(), 3000),
    );
}

/// 4000 assignments and 4000 `[[ name -eq 1 ]]`: each comparison used to
/// collect every assigned name.
#[test]
fn four_thousand_bare_name_comparisons() {
    check(
        "four_thousand_bare_name_comparisons",
        ("Analytics.hs", "checkNumberComparisons", "node"),
        || {
            let mut s = String::from("#!/bin/bash\n");
            for i in 0..4000 {
                s += &format!("var_{i}=1\n");
            }
            for i in 0..4000 {
                s += &format!("[[ var_{i} -eq 1 ]]\n");
            }
            s + "[[ unset_var -eq 1 ]]\n"
        },
        |codes| assert_eq!(codes, [2309]),
    );
}
