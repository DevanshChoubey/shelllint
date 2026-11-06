// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code)]
//! Helpers shared by the parity tests.

use std::path::{Path, PathBuf};

/// Path to a fixture script in `tests/fixtures`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The shellcheck-rust CLI binary the parity tests compare against.
///
/// Honours `SHELLCHECK_RUST_CLI`, then `$CARGO_TARGET_DIR/debug`, then the
/// workspace `target/debug`. Returns `None` when no binary is found; the
/// parity tests then skip with a message. Build it with
/// `cargo build -p shellcheck-rust` to run them.
pub fn cli_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SHELLCHECK_RUST_CLI") {
        return Some(PathBuf::from(p));
    }
    let target = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let bin = target.join("debug/shellcheck-rust");
    bin.exists().then_some(bin)
}

/// Returns the CLI path, or prints a skip notice and returns `None`.
pub fn require_cli(test: &str) -> Option<PathBuf> {
    let bin = cli_bin();
    if bin.is_none() {
        eprintln!("SKIPPED {test}: shellcheck-rust CLI binary not found; run `cargo build -p shellcheck-rust` (or set SHELLCHECK_RUST_CLI)");
    }
    bin
}

/// Runs the CLI with `args` on `file` and returns its stdout. The exit
/// status is ignored: 1 just means that issues were found.
pub fn run_cli(bin: &Path, args: &[&str], file: &Path) -> String {
    let out = std::process::Command::new(bin)
        .args(args)
        .arg(file)
        .output()
        .expect("failed to run the CLI");
    assert!(out.status.code() != Some(2) && out.status.code() != Some(4), "CLI failed: {:?}", out);
    String::from_utf8(out.stdout).expect("CLI output is UTF-8")
}
