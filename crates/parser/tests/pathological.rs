// SPDX-License-Identifier: GPL-3.0-or-later
// Parser performance regressions (port of ShellCheck Parser.hs, e11f964).
//
// Each input must parse in under TIME_LIMIT inside an address-space-capped
// child process (this test binary re-run with `--exact <name>` and
// `SC_PARSER_PATHOLOGICAL_CHILD` set; see crates/conformance/tests/pathological.rs).
// The inputs are sized so that the old quadratic behaviour took well over
// the limit:
// - many pending here-docs: every checkpoint deep-copied (and every restore
//   deep-compared) the pending here-doc list, each entry carrying a copy of
//   the context stack (1500 here-docs: ~30 s);
// - many parse problems: `ignore_problems_of` / `parse_forgetting_context`
//   cloned the whole SystemState, including every parse problem so far, once
//   per simple command (16000 lines with one problem each: ~18 s).

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use shellcheck_core::interface::{ParseSpec, mocked_system_interface, new_parse_spec};

const CHILD_ENV: &str = "SC_PARSER_PATHOLOGICAL_CHILD";
const TIME_LIMIT: Duration = Duration::from_secs(10);
const AS_LIMIT_KB: u64 = 4_000_000;

fn parse(script: &str) -> usize {
    let sys = mocked_system_interface(vec![]);
    let spec = ParseSpec { filename: "-".into(), script: script.into(), ..new_parse_spec() };
    // Parse on a big stack, as the checker does.
    shellcheck_parser::engine::with_stack(256 << 20, || shellcheck_parser::parse_script(&sys, &spec).comments.len())
}

fn check(name: &str, script: impl FnOnce() -> String) {
    if std::env::var_os(CHILD_ENV).is_some() {
        let script = script();
        let start = Instant::now();
        let n = parse(&script);
        println!("COMMENTS={n} PARSE_MS={}", start.elapsed().as_millis());
        return;
    }
    let start = Instant::now();
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("ulimit -v {AS_LIMIT_KB} && exec \"$0\" \"$@\""))
        .arg(&exe)
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
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
            panic!("{name}: took longer than {TIME_LIMIT:?}");
        }
    };
    let status = child.wait().unwrap();
    let elapsed = start.elapsed();
    assert!(status.success(), "{name}: child failed ({status}):\n{out}\n{err}");
    assert!(out.contains("COMMENTS="), "{name}: no result in child output:\n{out}\n{err}");
    eprintln!("{name}: {elapsed:?}");
    assert!(elapsed < TIME_LIMIT, "{name}: took {elapsed:?}");
}

#[test]
fn many_pending_here_docs() {
    check("many_pending_here_docs", || {
        let n = 1500;
        let mut s = String::from("#!/bin/sh\ncat");
        for i in 0..n {
            s.push_str(&format!(" <<E{i}"));
        }
        s.push('\n');
        for i in 0..n {
            s.push_str(&format!("x\nE{i}\n"));
        }
        s
    });
}

#[test]
fn many_parse_problems() {
    check("many_parse_problems", || {
        let mut s = String::from("#!/bin/sh\n");
        for _ in 0..16000 {
            s.push_str("sleep 1 &; echo b c d e f g h\n");
        }
        s
    });
}
