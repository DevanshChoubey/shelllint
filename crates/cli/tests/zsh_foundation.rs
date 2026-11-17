// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only tests (zsh is an extension).
//
// zsh is accepted at every entry point the other shells are: `--shell`,
// `--fallback-shell`, the `shell=` directive, the rc key, editorconfig, the
// shebang and the file name. Shells shelllint still does not support (fish,
// tcsh) stay rejected.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-zshf-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str], cwd: &Path) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(cwd)
        .env_remove("SHELLCHECK_OPTS")
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn clean() -> (i32, String, String) {
    (0, String::new(), String::new())
}

const SCRIPT: &str = "echo \"hi\"\n";
// `${x:0:1}` is SC3057 under sh and fine under bash, ksh and zsh.
const BASHISM: &str = "x=a\necho \"${x:0:1}\"\n";

fn write(d: &Path, name: &str, contents: &str) {
    std::fs::write(d.join(name), contents).unwrap();
}

#[test]
fn shell_flag() {
    let d = dir("flag");
    write(&d, "x", SCRIPT);
    assert_eq!(run(&["--norc", "-s", "zsh", "x"], &d), clean());
    assert_eq!(run(&["--norc", "--shell=zsh", "x"], &d), clean());
    // bash is the shell when zsh is not asked for, so a missing shebang is reported.
    assert!(run(&["--norc", "-f", "gcc", "x"], &d).1.contains("SC2148"));
}

#[test]
fn unsupported_shells_are_still_rejected() {
    let d = dir("unsup");
    write(&d, "x", SCRIPT);
    for sh in ["fish", "tcsh", "csh"] {
        let (code, _, err) = run(&["--norc", "-s", sh, "x"], &d);
        assert_eq!(code, 4, "{sh}");
        assert_eq!(err, format!("Unknown shell: {sh}\n"));
    }
}

#[test]
fn fallback_shell() {
    let d = dir("fallback");
    write(&d, "x", SCRIPT);
    assert_eq!(run(&["--norc", "--fallback-shell=zsh", "x"], &d), clean());
    assert_eq!(run(&["--norc", "--fallback-shell", "zsh", "x"], &d), clean());
    let (code, _, err) = run(&["--norc", "--fallback-shell=tcsh", "x"], &d);
    assert_eq!((code, err.as_str()), (4, "Unknown shell: tcsh\n"));
}

#[test]
fn shell_directive() {
    let d = dir("directive");
    write(&d, "x", &format!("# shellcheck shell=zsh\n{SCRIPT}"));
    assert_eq!(run(&["--norc", "x"], &d), clean());
    // An unknown shell in the directive is still SC1103.
    write(&d, "y", &format!("# shellcheck shell=tcsh\n{SCRIPT}"));
    assert!(run(&["--norc", "-f", "gcc", "y"], &d).1.contains("SC1103"));
}

#[test]
fn rc_key() {
    let d = dir("rc");
    write(&d, "x", SCRIPT);
    write(&d, ".shellcheckrc", "shell=zsh\n");
    assert_eq!(run(&["x"], &d), clean());
    // --strict-rc accepts the value.
    assert_eq!(run(&["--strict-rc", "x"], &d), clean());
    write(&d, ".shellcheckrc", "shell=tcsh\n");
    assert_eq!(run(&["--strict-rc", "x"], &d).0, 2);
}

#[test]
fn editorconfig() {
    let d = dir("editorconfig");
    write(&d, "x", SCRIPT);
    write(&d, ".editorconfig", "[x]\nshellcheck.shell=zsh\n");
    assert_eq!(run(&["x"], &d), clean());
    write(&d, ".editorconfig", "[x]\nshellcheck.shell=tcsh\n");
    assert!(run(&["-f", "gcc", "x"], &d).1.contains("SC1134"));
}

#[test]
fn shebangs() {
    let d = dir("shebang");
    for (i, sb) in ["#!/bin/zsh", "#!/usr/bin/env zsh", "#!/usr/bin/zsh -f", "#! /bin/zsh"].iter().enumerate() {
        let f = format!("s{i}");
        write(&d, &f, &format!("{sb}\n{SCRIPT}"));
        assert_eq!(run(&["--norc", "-f", "gcc", &f], &d), clean(), "{sb}");
    }
    // No more SC1071 for zsh; other unsupported shells keep it.
    write(&d, "t", &format!("#!/bin/tcsh\n{SCRIPT}"));
    assert!(run(&["--norc", "-f", "gcc", "t"], &d).1.contains("SC1071"));
    write(&d, "u", "#!/bin/zsh\n# shellcheck disable=SC1071\nfor f (a b); echo $f\n");
    assert_eq!(run(&["--norc", "-f", "gcc", "u"], &d), clean());
}

#[test]
fn file_names_infer_zsh() {
    let d = dir("names");
    for name in ["a.zsh", "a.zsh-theme", ".zshrc", ".zshenv", ".zprofile", ".zlogin", ".zlogout"] {
        write(&d, name, SCRIPT);
        // The checker's name table: a zsh name is a shell, so no SC2148.
        assert_eq!(run(&["--norc", name], &d), clean(), "{name}");
        // `--infer-shell-from-name` picks zsh over the fallback, which would be sh.
        write(&d, name, BASHISM);
        assert_eq!(run(&["--norc", "--infer-shell-from-name", "--fallback-shell=sh", name], &d), clean(), "{name}");
        // ...and without the name the fallback decides.
        write(&d, "plain", BASHISM);
        assert!(run(&["--norc", "-f", "gcc", "--fallback-shell=sh", "plain"], &d).1.contains("SC3057"));
    }
    // A name that only looks similar is not zsh.
    write(&d, "a.zshx", SCRIPT);
    assert!(run(&["--norc", "-f", "gcc", "a.zshx"], &d).1.contains("SC2148"));
}

#[test]
fn zsh_shebang_body_is_linted() {
    // Under `--shell=zsh` the same body is linted today.
    let d = dir("lintbody");
    write(&d, "x", "echo \"$undefined\"\n");
    assert!(run(&["--norc", "-f", "gcc", "--shell=zsh", "x"], &d).1.contains("SC2154"));
    // With only the shebang naming zsh it must be too.
    write(&d, "y", "#!/bin/zsh\necho \"$undefined\"\n");
    assert!(run(&["--norc", "-f", "gcc", "y"], &d).1.contains("SC2154"));
    // And `for f (a b); echo $f` (prop_readScript7) lints without a parse error.
    write(&d, "z", "#!/bin/zsh\nfor f (a b); echo $f\n");
    assert!(!run(&["--norc", "-f", "gcc", "z"], &d).1.contains("SC10"));
}
