// SPDX-License-Identifier: GPL-3.0-or-later
// CLI tests for `--bash-version`, the `bash-version=` rc key and the directive (an extension).

use std::path::PathBuf;
use std::process::Command;

const PIPE: &str = "#!/bin/bash\ndate |& tee out\n";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-bashver-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str], cwd: &PathBuf) -> (i32, String, String) {
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

fn setup(name: &str, script: &str) -> PathBuf {
    let d = dir(name);
    std::fs::write(d.join("x.sh"), script).unwrap();
    d
}

#[test]
fn off_by_default() {
    let d = setup("t1", PIPE);
    assert_eq!(run(&["--norc", "-f", "gcc", "x.sh"], &d), (0, String::new(), String::new()));
}

#[test]
fn flag() {
    let d = setup("t2", PIPE);
    let (code, out, _) = run(&["--norc", "--bash-version=3.2", "-f", "gcc", "x.sh"], &d);
    assert_eq!(code, 1);
    assert_eq!(out, "x.sh:2:6: warning: |& requires bash 4.0 (target is 3.2). [SC9104]\n");
    assert_eq!(run(&["--norc", "--bash-version=4.0", "-f", "gcc", "x.sh"], &d).0, 0);
    assert_eq!(run(&["--norc", "--bash-version=4", "-f", "gcc", "x.sh"], &d).0, 0);
    assert_eq!(run(&["--norc", "--bash-version", "3.2", "-f", "gcc", "x.sh"], &d).0, 1);
}

#[test]
fn flag_can_be_excluded() {
    let d = setup("t3", PIPE);
    assert_eq!(run(&["--norc", "--bash-version=3.2", "--exclude=SC9104", "x.sh"], &d).0, 0);
}

#[test]
fn invalid_flag_value_exits_3() {
    let d = setup("t4", PIPE);
    for bad in ["3.7", "2.0", "5.4", "6", "x", "", "3.2.57", "03.2"] {
        let (code, out, err) = run(&["--norc", &format!("--bash-version={bad}"), "x.sh"], &d);
        assert_eq!(code, 3, "{bad:?}");
        assert_eq!(out, "");
        assert!(err.contains("--bash-version"), "{err}");
    }
}

#[test]
fn rc_key() {
    let d = setup("t5", PIPE);
    std::fs::write(d.join(".shellcheckrc"), "bash-version=3.2\n").unwrap();
    let (code, out, _) = run(&["-f", "gcc", "x.sh"], &d);
    assert_eq!((code, out.contains("SC9104")), (1, true), "{out}");
    std::fs::write(d.join(".shellcheckrc"), "bash-version=4.4\n").unwrap();
    assert_eq!(run(&["-f", "gcc", "x.sh"], &d).0, 0);
    // --norc turns it off.
    std::fs::write(d.join(".shellcheckrc"), "bash-version=3.2\n").unwrap();
    assert_eq!(run(&["--norc", "x.sh"], &d).0, 0);
}

#[test]
fn directive() {
    let d = setup("t6", "#!/bin/bash\n# shellcheck bash-version=3.2\ndate |& tee out\n");
    let (code, out, _) = run(&["--norc", "-f", "gcc", "x.sh"], &d);
    assert_eq!((code, out.contains("SC9104")), (1, true), "{out}");
}

#[test]
fn precedence() {
    // flag > directive > rc
    let d = setup("t7", "#!/bin/bash\n# shellcheck bash-version=4.0\ndate |& tee out\n");
    std::fs::write(d.join(".shellcheckrc"), "bash-version=3.2\n").unwrap();
    assert_eq!(run(&["x.sh"], &d).0, 0, "directive beats rc");
    assert_eq!(run(&["--bash-version=3.2", "x.sh"], &d).0, 1, "flag beats directive");
    let d = setup("t8", "#!/bin/bash\n# shellcheck bash-version=3.2\ndate |& tee out\n");
    std::fs::write(d.join(".shellcheckrc"), "bash-version=5.0\n").unwrap();
    assert_eq!(run(&["x.sh"], &d).0, 1, "directive beats rc");
    assert_eq!(run(&["--bash-version=5.0", "x.sh"], &d).0, 0, "flag beats both");
    let d = setup("t9", PIPE);
    std::fs::write(d.join(".shellcheckrc"), "bash-version=5.0\n").unwrap();
    assert_eq!(run(&["--bash-version=3.2", "x.sh"], &d).0, 1, "flag beats rc");
}

#[test]
fn strict_rc_rejects_bad_value() {
    let d = setup("t10", "#!/bin/sh\ntrue\n");
    std::fs::write(d.join(".shellcheckrc"), "bash-version=3.7\n").unwrap();
    let (code, _, err) = run(&["--strict-rc", "x.sh"], &d);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("bash-version"), "{err}");
    // Otherwise a bad value is ignored, like the other rc keys.
    assert_eq!(run(&["x.sh"], &d).0, 0);
    for ok in ["3.2", "4", "\"5.3\"", "'4.4'"] {
        std::fs::write(d.join(".shellcheckrc"), format!("bash-version={ok}\n")).unwrap();
        assert_eq!(run(&["--strict-rc", "x.sh"], &d).0, 0, "{ok}");
    }
}

#[test]
fn bad_directive_value_is_sc1146() {
    let d = setup("t11", "#!/bin/bash\n# shellcheck bash-version=3.7\ndate |& tee out\n");
    let (code, out, _) = run(&["--norc", "-f", "gcc", "x.sh"], &d);
    assert_eq!(code, 1);
    assert!(out.contains("SC1146"), "{out}");
    assert!(!out.contains("SC9104"), "{out}");
}

#[test]
fn ignored_for_other_shells() {
    let d = setup("t12", "#!/bin/sh\ndate |& tee out\n");
    let (_, out, _) = run(&["--norc", "--bash-version=3.0", "-f", "gcc", "x.sh"], &d);
    assert!(!out.contains("SC91"), "{out}");
    let (_, out, _) = run(&["--norc", "--bash-version=3.0", "-s", "dash", "-f", "gcc", "x.sh"], &d);
    assert!(!out.contains("SC91"), "{out}");
}

#[test]
fn in_help_and_completions() {
    let d = dir("help");
    let (_, help, _) = run(&["--help"], &d);
    assert!(help.contains("--bash-version=X.Y"), "{help}");
    let (_, bash, _) = run(&["--completions=bash"], &d);
    assert!(bash.contains("--bash-version"), "{bash}");
}

#[test]
fn cache_key_includes_the_target() {
    let d = setup("t13", PIPE);
    let c = d.join("cache");
    let cache = format!("--cache-dir={}", c.display());
    assert_eq!(run(&["--norc", &cache, "x.sh"], &d).0, 0);
    assert_eq!(run(&["--norc", &cache, "--bash-version=3.2", "x.sh"], &d).0, 1);
    assert_eq!(run(&["--norc", &cache, "--bash-version=4.0", "x.sh"], &d).0, 0);
    assert_eq!(run(&["--norc", &cache, "--bash-version=3.2", "x.sh"], &d).0, 1);
}
