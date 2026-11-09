// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--cache-dir`, an extension beyond upstream ShellCheck.
#![cfg(unix)]

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(PartialEq, Eq, Debug)]
struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("shelllint-cache-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.0.join(name), contents).unwrap();
    }
    /// The entries in the cache directory: (path, inode, mtime ns).
    fn entries(&self) -> Vec<(PathBuf, u64, i64)> {
        let mut v: Vec<_> = std::fs::read_dir(self.0.join("cache"))
            .map(|rd| {
                rd.map(|e| {
                    let p = e.unwrap().path();
                    let m = std::fs::metadata(&p).unwrap();
                    (p, m.ino(), m.mtime_nsec() + m.mtime() * 1_000_000_000)
                })
                .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_in(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("xdg"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

const WARN: &str = "#!/bin/sh\necho $1\nx=$(ls)\n";

#[test]
fn hit_is_identical_to_miss() {
    let t = TempDir::new("hit");
    t.write("a.sh", WARN);
    for fmt in ["tty", "gcc", "json1", "checkstyle", "diff"] {
        let fresh = run_in(&t.0, &["--color=never", "-f", fmt, "a.sh"]);
        let args = ["--color=never", "-f", fmt, "--cache-dir=cache", "a.sh"];
        let miss = run_in(&t.0, &args);
        let before = t.entries();
        let hit = run_in(&t.0, &args);
        assert_eq!(miss, fresh, "{fmt}: miss differs from uncached");
        assert_eq!(hit, fresh, "{fmt}: hit differs from uncached");
        // A hit leaves the entry untouched (a miss would write a new file).
        assert_eq!(t.entries(), before, "{fmt}: entry rewritten on a hit");
    }
    assert_eq!(t.entries().len(), 1, "format and colour must not split entries");
}

#[test]
fn editing_the_file_misses() {
    let t = TempDir::new("edit");
    t.write("a.sh", WARN);
    let args = ["--cache-dir=cache", "a.sh"];
    let first = run_in(&t.0, &args);
    assert_eq!(first.code, 1);
    t.write("a.sh", "#!/bin/sh\necho \"$1\"\n");
    let second = run_in(&t.0, &args);
    assert_eq!(second, run_in(&t.0, &["a.sh"]));
    assert_ne!(second.stdout, first.stdout);
    assert_eq!(t.entries().len(), 2);
}

#[test]
fn editing_a_sourced_file_misses() {
    let t = TempDir::new("source");
    t.write("main.sh", "#!/bin/sh\n. ./lib.sh\n");
    t.write("lib.sh", "echo $1\n");
    let args = ["--cache-dir=cache", "-x", "-a", "-s", "sh", "main.sh"];
    let first = run_in(&t.0, &args);
    assert!(first.stdout.contains("SC2086"), "{}", first.stdout);
    let before = t.entries();
    assert_eq!(run_in(&t.0, &args), first);
    assert_eq!(t.entries(), before, "unchanged sourced file should hit");

    t.write("lib.sh", "echo \"$1\"\n");
    let second = run_in(&t.0, &args);
    assert_eq!(second, run_in(&t.0, &["-x", "-a", "-s", "sh", "main.sh"]));
    assert!(!second.stdout.contains("SC2086"), "{}", second.stdout);
    assert_ne!(t.entries(), before);
}

#[test]
fn changing_exclude_misses() {
    let t = TempDir::new("exclude");
    t.write("a.sh", WARN);
    let plain = ["--cache-dir=cache", "a.sh"];
    let excl = ["--cache-dir=cache", "-e", "SC2086", "a.sh"];
    let a = run_in(&t.0, &plain);
    let b = run_in(&t.0, &excl);
    assert!(a.stdout.contains("SC2086"));
    assert!(!b.stdout.contains("SC2086"));
    assert_eq!(t.entries().len(), 2);
    assert_eq!(run_in(&t.0, &plain), a);
    assert_eq!(run_in(&t.0, &excl), b);
    assert_eq!(t.entries().len(), 2);
}

#[test]
fn changing_the_rc_file_misses() {
    let t = TempDir::new("rc");
    t.write("a.sh", WARN);
    let args = ["--cache-dir=cache", "a.sh"];
    let a = run_in(&t.0, &args);
    assert!(a.stdout.contains("SC2086"));
    t.write(".shellcheckrc", "disable=SC2086\n");
    let b = run_in(&t.0, &args);
    assert!(!b.stdout.contains("SC2086"), "{}", b.stdout);
    assert_eq!(b, run_in(&t.0, &["a.sh"]));
}

#[test]
fn corrupt_entry_recovers() {
    let t = TempDir::new("corrupt");
    t.write("a.sh", WARN);
    let args = ["--cache-dir=cache", "a.sh"];
    let first = run_in(&t.0, &args);
    let entry = t.entries().remove(0).0;
    let good = std::fs::read(&entry).unwrap();
    let mut flipped = good.clone();
    let mid = flipped.len() / 2;
    flipped[mid] ^= 0xff;
    for bad in [b"garbage".to_vec(), Vec::new(), good[..good.len() - 5].to_vec(), flipped] {
        std::fs::write(&entry, &bad).unwrap();
        assert_eq!(run_in(&t.0, &args), first);
        assert_eq!(std::fs::read(&entry).unwrap(), good, "entry should be rewritten");
    }
}
