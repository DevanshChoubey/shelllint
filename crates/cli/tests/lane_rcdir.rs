//! `source-path=RCDIR` in a .shellcheckrc resolves to the rc file's directory.
use std::process::{Command, Stdio};

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-rcdir-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_in(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
    )
}

/// Layout: root/.shellcheckrc, root/lib/helper.sh (unused var is only seen when
/// the source is followed), root/src/deep/main.sh sourcing "helper.sh".
fn layout(name: &str, rc: &str) -> std::path::PathBuf {
    let d = tmpdir(name);
    std::fs::create_dir_all(d.join("lib")).unwrap();
    std::fs::create_dir_all(d.join("src/deep")).unwrap();
    std::fs::write(d.join(".shellcheckrc"), rc).unwrap();
    std::fs::write(d.join("lib/helper.sh"), "#!/bin/sh\nhelper_var=1\n").unwrap();
    std::fs::write(d.join("src/deep/main.sh"), "#!/bin/sh\n. helper.sh\necho \"$helper_var\"\n").unwrap();
    d
}

const FOUND: &str = "SC2154";

#[test]
fn rcdir_with_subdir_resolves_source() {
    let d = layout("sub", "source-path=RCDIR/lib\n");
    // Followed: helper_var is assigned, so no SC2154; and no SC1091.
    let (code, out) = run_in(&d, &["-x", "src/deep/main.sh"]);
    assert!(!out.contains("SC1091") && !out.contains(FOUND), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn rcdir_alone_is_the_rc_directory() {
    let d = layout("bare", "source-path=RCDIR\n");
    std::fs::write(d.join("helper.sh"), "#!/bin/sh\nhelper_var=1\n").unwrap();
    let (code, out) = run_in(&d.join("src"), &["-x", "deep/main.sh"]);
    assert!(!out.contains("SC1091"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn rcdir_is_not_the_script_or_cwd_directory() {
    let d = layout("cwd", "source-path=RCDIR/lib\n");
    // Run from a different directory, passing the script by path.
    let (code, out) = run_in(&d.join("src"), &["-x", "deep/main.sh"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn literal_path_starting_with_rcdir_is_unchanged() {
    let d = layout("lit", "source-path=RCDIRlib\n");
    std::fs::create_dir_all(d.join("RCDIRlib")).unwrap();
    std::fs::write(d.join("RCDIRlib/helper.sh"), "#!/bin/sh\nhelper_var=1\n").unwrap();
    // Not expanded, and relative to cwd it exists.
    let (code, out) = run_in(&d, &["-x", "src/deep/main.sh"]);
    assert_eq!(code, 0, "{out}");
    // Without a matching literal directory the source is not found.
    let d2 = layout("lit2", "source-path=RCDIRlib\n");
    let (_, out2) = run_in(&d2, &["-x", "src/deep/main.sh"]);
    assert!(out2.contains("SC1091"), "{out2}");
}

#[test]
fn without_rcdir_the_source_is_not_found() {
    let d = layout("none", "source-path=lib\n");
    let (code, out) = run_in(&d.join("src"), &["-x", "deep/main.sh"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("SC1091"), "{out}");
}
