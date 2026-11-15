//! `source ~/x` behaves like `source "$HOME/x"`: the prefix is dropped and the
//! rest is searched in the script directory and the source paths (#1845).
use std::process::{Command, Stdio};

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-tilde-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(dir: &std::path::Path, args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

fn setup(name: &str, line: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let d = tmpdir(name);
    let h = d.join("H");
    std::fs::create_dir_all(&h).unwrap();
    std::fs::write(h.join(".bashrc"), "x=1\n").unwrap();
    std::fs::create_dir_all(d.join("s")).unwrap();
    std::fs::write(d.join("s/a.sh"), format!("#!/bin/bash\n{line}\necho \"$x\"\n")).unwrap();
    (d, h)
}

#[test]
fn tilde_source_found_via_source_path() {
    let (d, h) = setup("found", "source ~/.bashrc");
    let o = run(&d.join("s"), &["-x", "-P", h.to_str().unwrap(), "a.sh"]);
    assert!(!o.contains("SC1090") && !o.contains("SC2154"), "{o}");
}

#[test]
fn tilde_source_without_path_is_1091() {
    let (d, _) = setup("nopath", "source ~/.bashrc");
    let o = run(&d.join("s"), &["-x", "a.sh"]);
    assert!(o.contains("SC1091") && !o.contains("SC1090"), "{o}");
}

#[test]
fn tilde_other_user_unchanged() {
    // Another user's home is not stripped: same as before (and as the oracle),
    // the literal path is tried and not found, so SC1091; -P never resolves it.
    let (d, h) = setup("other", "source ~other/.bashrc");
    let o = run(&d.join("s"), &["-x", "-P", h.to_str().unwrap(), "a.sh"]);
    assert!(o.contains("SC1091") && o.contains("SC2154"), "{o}");
}

#[test]
fn dollar_home_unchanged() {
    let (d, h) = setup("home", "source \"$HOME/.bashrc\"");
    let o = run(&d.join("s"), &["-x", "-P", h.to_str().unwrap(), "a.sh"]);
    assert!(!o.contains("SC1090") && !o.contains("SC2154"), "{o}");
    let o2 = run(&d.join("s"), &["-x", "a.sh"]);
    assert!(o2.contains("SC1091"), "{o2}");
}

#[test]
fn quoted_tilde_unchanged() {
    for line in ["source \"~/.bashrc\"", "source '~/.bashrc'"] {
        let (d, h) = setup("quoted", line);
        let o = run(&d.join("s"), &["-x", "-P", h.to_str().unwrap(), "a.sh"]);
        assert!(o.contains("SC1090"), "{line}: {o}");
    }
}

#[test]
fn directive_source_tilde_unchanged() {
    // The directive keeps 0.2.0's behaviour: `~` expands to the real home
    // directory. Point it at the test's own directory so the result doesn't
    // depend on the machine.
    let (d, h) = setup("directive", "f=unused\n# shellcheck source=~/.bashrc\nsource \"$f\"");
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["-x", "a.sh"])
        .current_dir(d.join("s"))
        .env_remove("SHELLCHECK_OPTS")
        .env("HOME", &h)
        .env("USERPROFILE", &h)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let o = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(!o.contains("SC1090") && !o.contains("SC1091") && !o.contains("SC2154"), "{o}");
}
