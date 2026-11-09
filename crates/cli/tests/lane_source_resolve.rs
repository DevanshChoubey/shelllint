// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `source-functions` / `--source-functions` and `--source-from-path`,
// extensions beyond upstream ShellCheck.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// A scratch directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("shelllint-srcres-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        TempDir(d)
    }
    fn write(&self, name: &str, contents: &str) {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_in(dir: &Path, args: &[&str], path: Option<&Path>) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shelllint"));
    cmd.args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(p) = path {
        cmd.env("PATH", p);
    }
    let o = cmd.output().unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

const WRAPPER: &str = "#!/bin/bash\nload lib.sh\necho \"$foo\"\n";

fn wrapper_dir(name: &str) -> TempDir {
    let d = TempDir::new(name);
    d.write("lib.sh", "foo=1\n");
    d.write("main.sh", WRAPPER);
    d
}

#[test]
fn wrapper_call_is_not_followed_by_default() {
    let d = wrapper_dir("default");
    let o = run_in(&d.0, &["-x", "-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("SC2154"), "{}", o.stdout);
}

#[test]
fn source_functions_flag_follows_wrapper() {
    let d = wrapper_dir("flag");
    let o = run_in(&d.0, &["-x", "--source-functions=load", "-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(o.stdout, "");
    // A list of names; the other name does not matter.
    let o = run_in(&d.0, &["-x", "--source-functions=use,load", "-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    // A different name does not follow `load`.
    let o = run_in(&d.0, &["-x", "--source-functions=use", "-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
}

#[test]
fn source_functions_in_rc_follows_wrapper() {
    let d = wrapper_dir("rc");
    d.write(".shellcheckrc", "source-functions=load,use\nexternal-sources=true\n");
    let o = run_in(&d.0, &["-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(o.stdout, "");
    // --norc turns it off again.
    let o = run_in(&d.0, &["--norc", "-x", "-f", "gcc", "main.sh"], None);
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
}

#[test]
fn source_functions_reports_missing_file_and_ignores_dynamic_args() {
    let d = TempDir::new("missing");
    d.write("main.sh", "#!/bin/bash\nload nothere.sh\nload \"$x\"\nload\n");
    let o = run_in(&d.0, &["-x", "--source-functions=load", "-f", "gcc", "main.sh"], None);
    assert!(o.stdout.contains("SC1091"), "{}", o.stdout);
    // Non-literal and missing arguments are left alone: no SC1090.
    assert!(!o.stdout.contains("SC1090"), "{}", o.stdout);
    // Without the option nothing is said about the wrapper.
    let o = run_in(&d.0, &["-x", "-f", "gcc", "main.sh"], None);
    assert!(!o.stdout.contains("SC1091"), "{}", o.stdout);
}

#[test]
fn source_from_path_finds_bare_filename_on_path() {
    let d = TempDir::new("frompath");
    d.write("bin/lib.sh", "foo=1\n");
    d.write("work/main.sh", "#!/bin/bash\nsource lib.sh\necho \"$foo\"\n");
    let bin = d.0.join("bin");
    let work = d.0.join("work");
    // Default: $PATH is not searched.
    let o = run_in(&work, &["-x", "-f", "gcc", "main.sh"], Some(&bin));
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("SC1091"), "{}", o.stdout);
    assert!(o.stdout.contains("SC2154"), "{}", o.stdout);
    // With the flag it is found.
    let o = run_in(&work, &["-x", "--source-from-path", "-f", "gcc", "main.sh"], Some(&bin));
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(o.stdout, "");
}

#[test]
fn source_from_path_only_for_bare_names_and_after_normal_search() {
    let d = TempDir::new("bare");
    d.write("bin/lib.sh", "foo=1\n");
    d.write("work/main.sh", "#!/bin/bash\nsource sub/lib.sh\necho \"$foo\"\n");
    let bin = d.0.join("bin");
    let work = d.0.join("work");
    let o = run_in(&work, &["-x", "--source-from-path", "-f", "gcc", "main.sh"], Some(&bin));
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("SC1091"), "{}", o.stdout);
    // A copy next to the script wins over $PATH.
    d.write("work/lib.sh", "bar=1\n");
    d.write("work/main2.sh", "#!/bin/bash\nsource lib.sh\necho \"$foo\" \"$bar\"\n");
    let o = run_in(&work, &["-x", "--source-from-path", "-f", "gcc", "main2.sh"], Some(&bin));
    assert!(o.stdout.contains("foo is referenced but not assigned"), "{}", o.stdout);
    assert!(!o.stdout.contains("bar is referenced"), "{}", o.stdout);
}
