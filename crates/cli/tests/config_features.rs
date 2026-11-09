// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the configuration features this port adds beyond ShellCheck:
// `severity=`, `~`/`$HOME` expansion, `external-vars` and nix-shell shebangs.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// A fresh directory; `home` inside it is used as HOME.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        let d = std::env::temp_dir()
            .join(format!("shelllint-cfg-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(d.join("home")).unwrap();
        std::fs::create_dir_all(d.join("work")).unwrap();
        Dir(d)
    }
    fn home(&self) -> PathBuf {
        self.0.join("home")
    }
    fn work(&self) -> PathBuf {
        self.0.join("work")
    }
    fn write(&self, rel: &str, contents: &str) {
        let p = self.work().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
    fn write_home(&self, rel: &str, contents: &str) {
        let p = self.home().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
    /// Runs in `work` with HOME (or none) set and the user's own config hidden.
    fn run(&self, args: &[&str], home: bool) -> Out {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_shelllint"));
        cmd.args(["-f", "gcc"])
            .args(args)
            .current_dir(self.work())
            .env_remove("SHELLCHECK_OPTS")
            .env_remove("HOME")
            .env_remove("USERPROFILE")
            .env("XDG_CONFIG_HOME", self.0.join("xdg"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if home {
            cmd.env(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, self.home());
        }
        let o = cmd.spawn().unwrap().wait_with_output().unwrap();
        Out {
            code: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn has(o: &Out, code: u32) -> bool {
    o.stdout.contains(&format!("SC{}", code))
}

fn _unused(_: &Path) {}

// ---- severity= ---------------------------------------------------------------

// SC2086 is info, SC2034 is warning.
const INFO_AND_WARN: &str = "#!/bin/sh\nunused=1\necho $1\n";

#[test]
fn severity_defaults_to_style_without_the_key() {
    let d = Dir::new();
    d.write("a.sh", INFO_AND_WARN);
    let o = d.run(&["a.sh"], true);
    assert!(has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
}

#[test]
fn severity_in_rc_file() {
    let d = Dir::new();
    d.write("a.sh", INFO_AND_WARN);
    d.write(".shellcheckrc", "severity=warning\n");
    let o = d.run(&["a.sh"], true);
    assert!(!has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    d.write(".shellcheckrc", "severity=error\n");
    let o = d.run(&["a.sh"], true);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert_eq!(o.stdout, "");
}

#[test]
fn severity_flag_beats_rc() {
    let d = Dir::new();
    d.write("a.sh", INFO_AND_WARN);
    d.write(".shellcheckrc", "severity=error\n");
    let o = d.run(&["--severity=style", "a.sh"], true);
    assert!(has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    d.write(".shellcheckrc", "severity=style\n");
    let o = d.run(&["-S", "warning", "a.sh"], true);
    assert!(!has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
}

#[test]
fn severity_file_directive_beats_rc_and_loses_to_flag() {
    let d = Dir::new();
    d.write("a.sh", &format!("#!/bin/sh\n# shellcheck severity=warning\n{}", &INFO_AND_WARN["#!/bin/sh\n".len()..]));
    // directive alone
    let o = d.run(&["a.sh"], true);
    assert!(!has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    // directive over rc (both directions)
    d.write(".shellcheckrc", "severity=error\n");
    let o = d.run(&["a.sh"], true);
    assert!(!has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    d.write(".shellcheckrc", "severity=style\n");
    let o = d.run(&["a.sh"], true);
    assert!(!has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    // flag over directive
    let o = d.run(&["--severity=style", "a.sh"], true);
    assert!(has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
}

#[test]
fn severity_invalid_value_is_reported() {
    let d = Dir::new();
    d.write("a.sh", "#!/bin/sh\ntrue\n");
    d.write(".shellcheckrc", "severity=loud\n");
    // Like every other invalid rc value (e.g. extended-analysis=maybe), the
    // note is not shown and the key has no effect.
    let o = d.run(&["a.sh"], true);
    assert_eq!((o.code, o.stdout.as_str()), (0, ""));
    // In a file directive it is reported
    d.write(".shellcheckrc", "");
    d.write("b.sh", "#!/bin/sh\n# shellcheck severity=loud\ntrue\n");
    let o = d.run(&["b.sh"], true);
    assert!(has(&o, 1147), "{}", o.stdout);
}

// ---- ~ and $HOME ---------------------------------------------------------------

const HELPER: &str = "#!/bin/sh\nhelper_var=1\n";

#[test]
fn source_path_directive_expands_home_forms() {
    for form in ["~/lib", "$HOME/lib", "${HOME}/lib"] {
        let d = Dir::new();
        d.write_home("lib/helper.sh", HELPER);
        d.write("a.sh", &format!("#!/bin/sh\n# shellcheck source-path={form}\n. helper.sh\necho \"$helper_var\"\n"));
        let o = d.run(&["-x", "a.sh"], true);
        assert!(!has(&o, 1091) && !has(&o, 2154), "{form}: {}", o.stdout);
    }
}

#[test]
fn source_path_directive_with_home_unset_is_unchanged() {
    let d = Dir::new();
    d.write_home("lib/helper.sh", HELPER);
    d.write("a.sh", "#!/bin/sh\n# shellcheck source-path=~/lib\n. helper.sh\n");
    let o = d.run(&["-x", "a.sh"], false);
    assert!(has(&o, 1091), "{}", o.stdout);
}

#[test]
fn source_path_in_rc_file_expands_home() {
    let d = Dir::new();
    d.write_home("lib/helper.sh", HELPER);
    d.write(".shellcheckrc", "source-path=~/lib\n");
    d.write("a.sh", "#!/bin/sh\n. helper.sh\necho \"$helper_var\"\n");
    let o = d.run(&["-x", "a.sh"], true);
    assert!(!has(&o, 1091) && !has(&o, 2154), "{}", o.stdout);
}

#[test]
fn source_directive_expands_home() {
    let d = Dir::new();
    d.write_home("lib/helper.sh", HELPER);
    d.write("a.sh", "#!/bin/sh\n# shellcheck source=~/lib/helper.sh\n. \"$1\"\necho \"$helper_var\"\n");
    let o = d.run(&["-x", "a.sh"], true);
    assert!(!has(&o, 1090) && !has(&o, 2154), "{}", o.stdout);
    // With HOME unset the text stays `~/lib/helper.sh`, which does not exist.
    let o = d.run(&["-x", "a.sh"], false);
    assert!(has(&o, 1090), "{}", o.stdout);
}

#[test]
fn source_path_flag_expands_home() {
    let d = Dir::new();
    d.write_home("lib/helper.sh", HELPER);
    d.write("a.sh", "#!/bin/sh\n. helper.sh\necho \"$helper_var\"\n");
    for flag in [vec!["-P", "~/lib"], vec!["--source-path=$HOME/lib"], vec!["--source-path=${HOME}/lib"]] {
        let mut args = vec!["-x"];
        args.extend(flag.iter());
        args.push("a.sh");
        let o = d.run(&args, true);
        assert!(!has(&o, 1091) && !has(&o, 2154), "{:?}: {}", flag, o.stdout);
    }
    let o = d.run(&["-x", "-P", "~/lib", "a.sh"], false);
    assert!(has(&o, 1091), "{}", o.stdout);
}

#[test]
fn unannotated_tilde_source_keeps_sc1090() {
    let d = Dir::new();
    d.write_home(".bashrc", "foo=1\n");
    d.write("a.sh", "#!/bin/bash\nsource ~/.bashrc\n");
    let o = d.run(&["-x", "a.sh"], true);
    assert!(has(&o, 1090), "{}", o.stdout);
}

// ---- external-vars ---------------------------------------------------------------

const USES: &str = "#!/bin/sh\necho \"$ci_name $ci_other $plain\"\n";

fn codes_2154(o: &Out) -> Vec<String> {
    o.stdout
        .lines()
        .filter(|l| l.contains("SC2154"))
        .map(|l| l.split("warning: ").nth(1).unwrap_or(l).split(' ').next().unwrap().to_string())
        .collect()
}

#[test]
fn external_vars_off_by_default() {
    let d = Dir::new();
    d.write("a.sh", USES);
    let o = d.run(&["a.sh"], true);
    assert_eq!(codes_2154(&o), ["ci_name", "ci_other", "plain"], "{}", o.stdout);
}

#[test]
fn external_vars_flag_exact_glob_and_repeat() {
    let d = Dir::new();
    d.write("a.sh", USES);
    let o = d.run(&["--external-vars=plain", "a.sh"], true);
    assert_eq!(codes_2154(&o), ["ci_name", "ci_other"], "{}", o.stdout);
    let o = d.run(&["--external-vars=ci_*", "a.sh"], true);
    assert_eq!(codes_2154(&o), ["plain"], "{}", o.stdout);
    let o = d.run(&["--external-vars=ci_name", "--external-vars=plain,ci_other", "a.sh"], true);
    assert!(codes_2154(&o).is_empty(), "{}", o.stdout);
    // A glob needs the prefix to match: `c*` does match, `x_*` does not.
    let o = d.run(&["--external-vars=x_*", "a.sh"], true);
    assert_eq!(codes_2154(&o).len(), 3, "{}", o.stdout);
}

#[test]
fn external_vars_invalid_name_is_a_usage_error() {
    let d = Dir::new();
    d.write("a.sh", USES);
    let o = d.run(&["--external-vars=a-b", "a.sh"], true);
    assert_eq!(o.code, 3, "{}", o.stderr);
    assert!(o.stderr.contains("a-b"), "{}", o.stderr);
}

#[test]
fn external_vars_rc_and_directive_add_up() {
    let d = Dir::new();
    d.write("a.sh", USES);
    d.write(".shellcheckrc", "external-vars=plain\n");
    let o = d.run(&["a.sh"], true);
    assert_eq!(codes_2154(&o), ["ci_name", "ci_other"], "{}", o.stdout);
    // a directive in the middle of the file applies to the whole file
    d.write("b.sh", "#!/bin/sh\necho \"$ci_name\"\n# shellcheck external-vars=ci_*\necho \"$ci_other $plain\"\n");
    let o = d.run(&["b.sh"], true);
    assert!(codes_2154(&o).is_empty(), "{}", o.stdout);
    // all three sources together
    d.write("c.sh", "#!/bin/sh\n# shellcheck external-vars=ci_name,ci_other\necho \"$ci_name $ci_other $plain $zed\"\n");
    let o = d.run(&["--external-vars=zed", "c.sh"], true);
    assert!(codes_2154(&o).is_empty(), "{}", o.stdout);
}

#[test]
fn external_vars_only_affects_sc2154() {
    let d = Dir::new();
    // `x` is declared external but is still unquoted and ends up unused-looking
    d.write("a.sh", "#!/bin/sh\necho $x\nunused=1\n");
    let base = d.run(&["a.sh"], true);
    let o = d.run(&["--external-vars=x", "a.sh"], true);
    assert!(has(&base, 2154) && !has(&o, 2154));
    assert!(has(&o, 2086) && has(&o, 2034), "{}", o.stdout);
    let strip = |s: &str| s.lines().filter(|l| !l.contains("SC2154")).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(&base.stdout), strip(&o.stdout));
}

// ---- nix-shell shebangs ----------------------------------------------------------

fn nix(first: &str, rest: &str) -> String {
    format!("{first}\n{rest}")
}

#[test]
fn nix_shell_shebang_without_i_is_bash() {
    let d = Dir::new();
    for first in ["#!/usr/bin/env nix-shell", "#!/nix/store/abc-nix/bin/nix-shell"] {
        d.write("a.sh", &nix(first, "#! nix-shell -p git\necho \"$BASH_VERSION\"\n"));
        let o = d.run(&["a.sh"], true);
        assert!(!has(&o, 1008) && !has(&o, 1071), "{first}: {}", o.stdout);
        // Bash features are accepted (no SC3xxx sh warnings)
        assert_eq!(o.stdout, "", "{first}");
    }
    // not even a nix-shell line
    d.write("b.sh", "#!/usr/bin/env nix-shell\ntrue\n");
    assert_eq!(d.run(&["b.sh"], true).stdout, "");
}

#[test]
fn nix_shell_i_selects_the_shell() {
    let d = Dir::new();
    let body = "x=(1 2)\n";
    for (line, bash_ok) in [
        ("#! nix-shell -i bash -p git", true),
        ("#! nix-shell -i \"bash -x\"", true),
        ("#! nix-shell -p git -i dash", false),
        ("#! nix-shell -i sh", false),
    ] {
        d.write("a.sh", &nix("#!/usr/bin/env nix-shell", &format!("{line}\n{body}")));
        let o = d.run(&["a.sh"], true);
        assert!(!has(&o, 1008), "{line}: {}", o.stdout);
        assert_eq!(!has(&o, 3054) && !has(&o, 3030), bash_ok, "{line}: {}", o.stdout);
    }
}

#[test]
fn nix_shell_with_a_non_shell_interpreter_keeps_sc1008() {
    let d = Dir::new();
    d.write("a.sh", &nix("#!/usr/bin/env nix-shell", "#! nix-shell -i python3 -p python3\nprint(1)\n"));
    let o = d.run(&["a.sh"], true);
    assert!(has(&o, 1008), "{}", o.stdout);
}

#[test]
fn nix_shell_shebang_lines_must_be_contiguous() {
    let d = Dir::new();
    // The `-i dash` line is after a blank line, so it is not part of the run.
    d.write("a.sh", &nix("#!/usr/bin/env nix-shell", "#! nix-shell -p git\n\n#! nix-shell -i dash\nx=(1 2)\n"));
    let o = d.run(&["a.sh"], true);
    assert!(!has(&o, 1008) && !has(&o, 3030), "{}", o.stdout);
}

#[test]
fn nix_shell_loses_to_shell_flag_and_directive() {
    let d = Dir::new();
    d.write("a.sh", &nix("#!/usr/bin/env nix-shell", "#! nix-shell -i bash\nx=(1 2)\n"));
    let o = d.run(&["--shell=dash", "a.sh"], true);
    assert!(has(&o, 3030) || has(&o, 3054), "{}", o.stdout);
    assert!(!has(&o, 1008));
    d.write(
        "b.sh",
        &nix("#!/usr/bin/env nix-shell", "#! nix-shell -i bash\n# shellcheck shell=dash\nx=(1 2)\n"),
    );
    let o = d.run(&["b.sh"], true);
    assert!(has(&o, 3030) || has(&o, 3054), "{}", o.stdout);
}

#[test]
fn other_unknown_shebangs_are_unchanged() {
    let d = Dir::new();
    d.write("a.sh", "#!/usr/bin/env nixos-run\n#! nix-shell -i bash\ntrue\n");
    let o = d.run(&["a.sh"], true);
    assert!(has(&o, 1008), "{}", o.stdout);
}
