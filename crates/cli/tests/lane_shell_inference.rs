// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for --fallback-shell, --infer-shell-from-name and
// --skip-unsupported-shebang (extensions beyond ShellCheck).

use std::process::{Command, Stdio};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// A file called `name` in a directory of its own (so no rc file is found).
fn file(name: &str, contents: &str) -> String {
    let dir = std::env::temp_dir().join(format!("shelllint-lsi-{}-{}", std::process::id(), name.replace('/', "_")));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, contents).unwrap();
    p.to_string_lossy().into_owned()
}

// ${x:0:1} is a bashism: SC3057 under sh, fine under bash and ksh.
const BASHISM: &str = "x=a\necho \"${x:0:1}\"\n";

fn assert_clean(o: &Out) {
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, "", ""));
}

#[test]
fn defaults_unchanged() {
    let f = file("plain", BASHISM);
    let o = run(&["--norc", "-f", "gcc", &f]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC2148"), "{}", o.stdout);
}

#[test]
fn fallback_used_when_no_shell_given() {
    let f = file("plain", BASHISM);
    assert_clean(&run(&["--norc", "--fallback-shell=bash", &f]));
    let o = run(&["--norc", "-f", "gcc", "--fallback-shell=sh", &f]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC3057") && !o.stdout.contains("SC2148"), "{}", o.stdout);
    // space-separated form
    assert_clean(&run(&["--norc", "--fallback-shell", "bash", &f]));
}

#[test]
fn fallback_does_not_override_shebang_or_directive() {
    let f = file("sb", &format!("#!/bin/bash\n{}", BASHISM));
    assert_clean(&run(&["--norc", "--fallback-shell=sh", &f]));
    let f = file("dir", &format!("# shellcheck shell=bash\n{}", BASHISM));
    assert_clean(&run(&["--norc", "--fallback-shell=sh", &f]));
    // ...but --shell does override the shebang, unlike a fallback.
    let f = file("sb2", &format!("#!/bin/bash\n{}", BASHISM));
    let o = run(&["--norc", "-f", "gcc", "--shell=sh", "--fallback-shell=bash", &f]);
    assert!(o.stdout.contains("SC3057"), "{}", o.stdout);
}

#[test]
fn fallback_does_not_override_shell_flag() {
    let f = file("plain", BASHISM);
    let o = run(&["--norc", "-f", "gcc", "--shell=sh", "--fallback-shell=bash", &f]);
    assert!(o.stdout.contains("SC3057"), "{}", o.stdout);
}

#[test]
fn unknown_fallback_shell_is_rejected() {
    let f = file("plain", BASHISM);
    let o = run(&["--norc", "--fallback-shell=zsh", &f]);
    assert_eq!(o.code, 4);
    assert!(o.stderr.contains("Unknown shell: zsh"), "{}", o.stderr);
}

#[test]
fn infer_from_name() {
    for name in
        [".bashrc", ".bash_profile", ".bash_login", ".bash_logout", ".bash_aliases", "x.bash", ".kshrc", "x.ksh"]
    {
        let f = file(name, BASHISM);
        let plain = run(&["--norc", "-f", "gcc", &f]);
        // .ksh/.bash suffixes are already handled by the checker (no SC2148); the rest report it.
        let _ = plain;
        assert_clean(&run(&["--norc", "--infer-shell-from-name", &f]));
    }
    for name in [".profile", ".shrc"] {
        let f = file(name, BASHISM);
        let o = run(&["--norc", "-f", "gcc", "--infer-shell-from-name", &f]);
        assert!(o.stdout.contains("SC3057") && !o.stdout.contains("SC2148"), "{}: {}", name, o.stdout);
    }
    // Without the flag a dotfile still gets SC2148.
    let f = file(".bashrc", BASHISM);
    assert!(run(&["--norc", "-f", "gcc", &f]).stdout.contains("SC2148"));
    // An unrecognised name changes nothing.
    let f = file("plain", BASHISM);
    assert!(run(&["--norc", "-f", "gcc", "--infer-shell-from-name", &f]).stdout.contains("SC2148"));
}

#[test]
fn infer_precedes_fallback_but_not_shebang() {
    let f = file(".bashrc", BASHISM);
    assert_clean(&run(&["--norc", "--infer-shell-from-name", "--fallback-shell=sh", &f]));
    let f = file(".profile", BASHISM);
    assert!(
        run(&["--norc", "-f", "gcc", "--infer-shell-from-name", "--fallback-shell=bash", &f]).stdout.contains("SC3057")
    );
    // fallback applies where the name tells nothing
    let f = file("plain", BASHISM);
    assert_clean(&run(&["--norc", "--infer-shell-from-name", "--fallback-shell=bash", &f]));
    // a shebang beats the name
    let f = file(".bashrc", &format!("#!/bin/sh\n{}", BASHISM));
    assert!(run(&["--norc", "-f", "gcc", "--infer-shell-from-name", &f]).stdout.contains("SC3057"));
}

#[test]
fn skip_non_shell_shebang() {
    let f = file("py", "#!/usr/bin/env python3\nprint('hi')\n");
    let o = run(&["--norc", "-f", "gcc", &f]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC1071"), "{}", o.stdout);
    assert_clean(&run(&["--norc", "--skip-unsupported-shebang", &f]));
    let f = file("unk", "#!/usr/bin/env node\nconsole.log(1)\n");
    assert!(run(&["--norc", "-f", "gcc", &f]).stdout.contains("SC1008"));
    assert_clean(&run(&["--norc", "--skip-unsupported-shebang", &f]));
}

#[test]
fn skip_file_with_no_recognisable_shell() {
    let f = file("plain", "echo $x\n");
    assert_eq!(run(&["--norc", &f]).code, 1);
    assert_clean(&run(&["--norc", "--skip-unsupported-shebang", &f]));
    // a fallback gives it a shell, so it is checked
    let o = run(&["--norc", "-f", "gcc", "--skip-unsupported-shebang", "--fallback-shell=bash", &f]);
    assert!(o.stdout.contains("SC2086"), "{}", o.stdout);
}

#[test]
fn skip_leaves_real_shell_scripts_alone() {
    let f = file("good", "#!/bin/bash\necho $x\n");
    let o = run(&["--norc", "-f", "gcc", "--skip-unsupported-shebang", &f]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC2086"), "{}", o.stdout);
    // --shell and a directive both mean a shell is given
    let f = file("py2", "#!/usr/bin/env python3\necho $x\n");
    let o = run(&["--norc", "-f", "gcc", "--skip-unsupported-shebang", "--shell=bash", &f]);
    assert!(o.stdout.contains("SC2086"), "{}", o.stdout);
    let f = file("dir2", "# shellcheck shell=bash\necho $x\n");
    let o = run(&["--norc", "-f", "gcc", "--skip-unsupported-shebang", &f]);
    assert!(o.stdout.contains("SC2086"), "{}", o.stdout);
}

#[test]
fn skip_mixed_files_keeps_exit_code_of_the_rest() {
    let py = file("m.py", "#!/usr/bin/python\nprint(1)\n");
    let ok = file("m.sh", "#!/bin/sh\ntrue\n");
    assert_eq!(run(&["--norc", "--skip-unsupported-shebang", &py, &ok]).code, 0);
}

#[test]
fn abbreviations_of_existing_options_still_work() {
    let f = file("plain", "echo $x\n");
    let o = run(&["--norc", "--in=SC2148", "-f", "gcc", &f]);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC2148") && !o.stdout.contains("SC2086"), "{}", o.stdout);
}
