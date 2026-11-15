// SPDX-License-Identifier: GPL-3.0-or-later
// CLI tests for Jinja template mode (`--template=jinja`, `template=jinja`, `.j2` names).

use std::path::PathBuf;
use std::process::Command;

const T1306: &str = "#!/bin/bash\nNAME={{ app_name }}\nport=\"{{ port | default(80) }}\"\n{% if enable_x %}\necho \"x on\"\n{% endif %}\n{# comment #}\necho \"$NAME $port\"\n";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shelllint-jinja-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str], cwd: &PathBuf) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .current_dir(cwd)
        .env_remove("SHELLCHECK_OPTS")
        .output()
        .unwrap();
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn jinja_auto_for_j2() {
    let d = dir("auto");
    std::fs::write(d.join("x.sh.j2"), T1306).unwrap();
    for name in ["x.sh.j2", "x.jinja", "x.jinja2"] {
        std::fs::write(d.join(name), T1306).unwrap();
        assert_eq!(run(&["--norc", "-f", "gcc", name], &d), (0, String::new()), "{name}");
    }
}

#[test]
fn jinja_flag_and_directive() {
    let d = dir("flag");
    std::fs::write(d.join("x.sh"), T1306).unwrap();
    let (code, out) = run(&["--norc", "-f", "gcc", "x.sh"], &d);
    assert_eq!(code, 1);
    assert!(out.contains("SC1083") || out.contains("SC1054") || out.contains("SC1009"), "{out}");
    assert_eq!(run(&["--norc", "--template=jinja", "-f", "gcc", "x.sh"], &d), (0, String::new()));
    let with_dir = T1306.replacen("#!/bin/bash\n", "#!/bin/bash\n# shellcheck template=jinja\n", 1);
    std::fs::write(d.join("y.sh"), with_dir).unwrap();
    assert_eq!(run(&["--norc", "-f", "gcc", "y.sh"], &d), (0, String::new()));
}

#[test]
fn jinja_rc_key() {
    let d = dir("rc");
    std::fs::write(d.join("x.sh"), T1306).unwrap();
    std::fs::write(d.join(".shellcheckrc"), "template=jinja\n").unwrap();
    assert_eq!(run(&["--strict-rc", "-f", "gcc", "x.sh"], &d), (0, String::new()));
    assert_eq!(run(&["--norc", "-f", "gcc", "x.sh"], &d).0, 1);
}

#[test]
fn jinja_bad_value() {
    let d = dir("bad");
    std::fs::write(d.join("x.sh"), "echo\n").unwrap();
    assert_eq!(run(&["--template=mako", "x.sh"], &d).0, 4);
}

#[test]
fn jinja_masks_inside_single_quotes_only_in_template_mode() {
    let d = dir("quote");
    let src = "#!/bin/bash\necho '{{ v }}' $x\n";
    std::fs::write(d.join("a.sh.j2"), src).unwrap();
    std::fs::write(d.join("a.sh"), src).unwrap();
    // Masked: the quoted text is plain underscores either way, the finding is on $x.
    let (_, j2) = run(&["--norc", "-f", "gcc", "a.sh.j2"], &d);
    assert!(j2.contains("SC2086"), "{j2}");
    // A masked `{{ v }}` outside quotes becomes one word, not a syntax error.
    std::fs::write(d.join("b.sh.j2"), "#!/bin/bash\necho {{ v }}\n").unwrap();
    assert_eq!(run(&["--norc", "-f", "gcc", "b.sh.j2"], &d), (0, String::new()));
    // Not a template: output is what it was before.
    std::fs::write(d.join("b.sh"), "#!/bin/bash\necho {{ v }}\n").unwrap();
    let (code, out) = run(&["--norc", "-f", "gcc", "b.sh"], &d);
    assert_eq!(code, 1, "{out}");
    let (_, plain) = run(&["--norc", "-f", "gcc", "a.sh"], &d);
    assert!(plain.contains("SC2086"), "{plain}");
}

#[test]
fn jinja_columns_unchanged() {
    let d = dir("cols");
    std::fs::write(d.join("x.sh.j2"), "#!/bin/bash\necho {{ a }} $y\n").unwrap();
    std::fs::write(d.join("x.sh"), "#!/bin/bash\necho aaaaaaa $y\n").unwrap();
    let col = |f: &str| {
        let (_, out) = run(&["--norc", "-f", "gcc", f], &d);
        out.lines().find(|l| l.contains("SC2086")).map(|l| l.split(':').nth(2).unwrap().to_string()).unwrap_or_default()
    };
    assert_eq!(col("x.sh.j2"), col("x.sh"));
    assert!(!col("x.sh.j2").is_empty());
}

#[test]
fn jinja_in_help_and_completions() {
    let d = dir("help");
    let (_, help) = run(&["--help"], &d);
    assert!(help.contains("--template"), "{help}");
    let (_, bash) = run(&["--completions=bash"], &d);
    assert!(bash.contains("--template"), "{bash}");
}
