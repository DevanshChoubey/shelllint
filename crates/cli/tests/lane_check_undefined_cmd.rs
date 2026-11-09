// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the optional check `undefined-command` (SC9004), a shelllint extension.
// Every run uses a controlled PATH made of a temporary directory.

use std::io::Write;
use std::process::{Command, Stdio};

fn path_dir(tag: &str, tools: &[(&str, bool)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shelllint-undef-{}-{tag}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    for (name, exec) in tools {
        let f = dir.join(name);
        std::fs::write(&f, "#!/bin/sh\n").unwrap();
        #[cfg(not(unix))]
        let _ = exec;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(if *exec { 0o755 } else { 0o644 })).unwrap();
        }
    }
    dir
}

/// Lint `script` on stdin with `PATH=path`; returns (exit code, stdout).
fn lint(path: &std::path::Path, args: &[&str], script: &str) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .args(["-f", "gcc", "-"])
        .env_remove("SHELLCHECK_OPTS")
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().unwrap().write_all(script.as_bytes());
    let o = child.wait_with_output().unwrap();
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn sc9004(out: &str) -> Vec<&str> {
    out.lines().filter(|l| l.contains("SC9004")).collect()
}

#[test]
fn off_by_default() {
    let dir = path_dir("off", &[]);
    let (code, out) = lint(&dir, &[], "#!/bin/sh\nnosuchtool_a --x\n");
    assert_eq!((code, out.as_str()), (0, ""));
}

#[test]
fn flags_missing_command_once_per_name() {
    let dir = path_dir("once", &[]);
    let script = "#!/bin/sh\nnosuchtool_a\nnosuchtool_a again\necho hi | nosuchtool_b\n";
    let (code, out) = lint(&dir, &["--enable=undefined-command"], script);
    assert_eq!(code, 1, "{out}");
    let hits = sc9004(&out);
    assert_eq!(hits.len(), 2, "{out}");
    assert!(hits[0].starts_with("-:2:1: note:") && hits[0].contains("'nosuchtool_a'"), "{out}");
    assert!(hits[1].starts_with("-:4:") && hits[1].contains("'nosuchtool_b'"), "{out}");
}

#[test]
fn quiet_for_path_builtin_function_alias_slash_and_expansions() {
    let dir = path_dir("quiet", &[("mytool", true)]);
    let script = "#!/bin/bash\n\
        mytool --x\n\
        cd /tmp; [[ -n x ]] && echo ok; if true; then :; fi\n\
        helper() { :; }\n\
        helper\n\
        alias ll='ls -l'\n\
        ll\n\
        ./local_script\n\
        /opt/x/bin/tool\n\
        \"$CMD\" arg\n\
        $(echo x) arg\n\
        X=1\n";
    // Other (default) checks may still fire on this script; only SC9004 matters.
    let (_, out) = lint(&dir, &["--enable=undefined-command"], script);
    assert!(sc9004(&out).is_empty(), "{out}");
}

#[test]
fn non_executable_file_does_not_count() {
    let dir = path_dir("noexec", &[("plainfile", false)]);
    let (_, out) = lint(&dir, &["--enable=undefined-command"], "#!/bin/sh\nplainfile\n");
    #[cfg(unix)]
    assert_eq!(sc9004(&out).len(), 1, "{out}");
    #[cfg(not(unix))]
    assert!(sc9004(&out).is_empty(), "{out}");
}

#[test]
fn result_depends_on_path() {
    let with = path_dir("with", &[("sometool", true)]);
    let without = path_dir("without", &[]);
    let script = "#!/bin/sh\nsometool\n";
    assert!(sc9004(&lint(&with, &["--enable=undefined-command"], script).1).is_empty());
    assert_eq!(sc9004(&lint(&without, &["--enable=undefined-command"], script).1).len(), 1);
}

#[test]
fn listed_with_description_mentioning_machine() {
    let dir = path_dir("list", &[]);
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint")).arg("--list-optional").env("PATH", &dir).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("name:    undefined-command"), "{out}");
    assert!(out.contains("depends on the machine"), "{out}");
}
