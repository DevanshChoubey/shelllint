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
fn tilde_source_without_path_is_1090() {
    let (d, _) = setup("nopath", "source ~/.bashrc");
    let o = run(&d.join("s"), &["-x", "a.sh"]);
    assert!(o.contains("SC1090") && !o.contains("SC1091"), "{o}");
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

/// Output of `shelllint -f gcc` on `script` written to `a.sh` next to a real `x.sh`
/// and `dir/f.sh`, for the given shell and extra flags.
fn gcc(name: &str, shell: &str, script: &str, extra: &[&str]) -> String {
    let d = tmpdir(name);
    std::fs::write(d.join("x.sh"), "echo hi\n").unwrap();
    std::fs::create_dir_all(d.join("dir")).unwrap();
    std::fs::write(d.join("dir/f.sh"), "echo hi\n").unwrap();
    std::fs::write(d.join("a.sh"), format!("{script}\n")).unwrap();
    let mut args = vec!["-f", "gcc", "-s", shell];
    args.extend_from_slice(extra);
    args.push("a.sh");
    run(&d, &args).replace("a.sh:", "-:")
}

const SC1090: &str = "ShellCheck can't follow non-constant source. Use a directive to specify location. [SC1090]";

#[test]
fn unquoted_tilde_source_is_sc1090_warning_like_the_oracle() {
    // Column checked against the oracle: `. ~/x.sh` is col 3, `source ~/...` col 8.
    for (i, sh) in ["sh", "bash"].into_iter().enumerate() {
        for (j, (script, col)) in
            [(". ~/x.sh", 3), ("source ~/.bashrc", 8), ("source ~/dir/f.sh", 8)].into_iter().enumerate()
        {
            // With -x a file that exists next to the script is followed (the 0.3.0 feature), so
            // the -x run names a file that does not exist: the oracle never follows `~/`.
            for (k, extra) in [vec![], vec!["-x"]].into_iter().enumerate() {
                let script = if k == 1 { script.replace("x.sh", "gone.sh").replace("f.sh", "gone.sh") } else { script.to_string() };
                let o = gcc(&format!("u{i}{j}{k}"), sh, &script, &extra);
                let want = format!("-:1:{col}: warning: {SC1090}");
                assert!(o.lines().any(|l| l == want), "{sh} {extra:?} {script}: {o}");
                assert!(!o.contains("SC1091"), "{sh} {extra:?} {script}: {o}");
            }
        }
    }
}

#[test]
fn tilde_source_sc1090_exit_status_under_severity_warning() {
    let d = tmpdir("exit");
    std::fs::write(d.join("a.sh"), "#!/bin/sh\n. ~/x.sh\n").unwrap();
    let st = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["-S", "warning", "a.sh"])
        .current_dir(&d)
        .env_remove("SHELLCHECK_OPTS")
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(1));
}

#[test]
fn tilde_source_with_x_and_p_still_follows_in_bash_and_sh() {
    for sh in ["sh", "bash"] {
        let d = tmpdir(&format!("follow-{sh}"));
        std::fs::create_dir_all(d.join("H/dir")).unwrap();
        std::fs::write(d.join("H/x.sh"), "y=1\n").unwrap();
        std::fs::write(d.join("H/dir/f.sh"), "y=1\n").unwrap();
        for line in [". ~/x.sh", ". ~/dir/f.sh"] {
            std::fs::write(d.join("a.sh"), format!("{line}\necho \"$y\"\n")).unwrap();
            let o = run(&d, &["-x", "-P", d.join("H").to_str().unwrap(), "-s", sh, "a.sh"]);
            assert!(!o.contains("SC1090") && !o.contains("SC1091") && !o.contains("SC2154"), "{sh} {line}: {o}");
            // Without -x the same script is not followed.
            let o = run(&d, &["-P", d.join("H").to_str().unwrap(), "-s", sh, "a.sh"]);
            assert!(o.contains("SC1090"), "{sh} {line}: {o}");
        }
    }
}

#[test]
fn bare_tilde_and_other_user_tilde_are_sc1091_like_the_oracle() {
    // Oracle: `. ~` and `source ~user/x` give SC1091 note naming the literal path.
    let o = gcc("bare", "bash", ". ~", &[]);
    assert!(o.contains("-:1:3: note: Not following: ~ was not specified as input"), "{o}");
    let o = gcc("user", "bash", "source ~user/x", &[]);
    assert!(o.contains("-:1:8: note: Not following: ~user/x was not specified as input"), "{o}");
    let o = gcc("root", "sh", "source ~root/z", &[]);
    assert!(o.contains("-:1:8: note: Not following: ~root/z was not specified as input"), "{o}");
}

#[test]
fn quoted_tilde_source_is_sc1090_with_sc2088() {
    let o = gcc("quoted-gcc", "bash", "source \"~/x\"", &[]);
    assert!(o.contains(&format!("-:1:8: warning: {SC1090}")), "{o}");
    assert!(o.contains("-:1:9: warning: Tilde does not expand in quotes. Use $HOME. [SC2088]"), "{o}");
}
