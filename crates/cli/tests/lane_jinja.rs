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
    // `$v` in single quotes is SC2016 as plain shell; masked, it is not there.
    let src = "#!/bin/bash\necho '{{ $v }}'\n";
    std::fs::write(d.join("a.sh.j2"), src).unwrap();
    std::fs::write(d.join("a.sh"), src).unwrap();
    let (_, plain) = run(&["--norc", "-f", "gcc", "a.sh"], &d);
    assert!(plain.contains("SC2016"), "{plain}");
    assert_eq!(run(&["--norc", "-f", "gcc", "a.sh.j2"], &d), (0, String::new()));
    // A masked `{{ v }}` outside quotes becomes one word, not a syntax error.
    std::fs::write(d.join("b.sh.j2"), "#!/bin/bash\necho {{ v }}\n").unwrap();
    assert_eq!(run(&["--norc", "-f", "gcc", "b.sh.j2"], &d), (0, String::new()));
}

#[test]
fn plain_sh_with_braces_matches_oracle() {
    let oracle = "/media/devansh/Experiment/playground/shellcheck-rust/bench/.bin/shellcheck-hs";
    if !std::path::Path::new(oracle).exists() {
        return;
    }
    let d = dir("oracle");
    for (i, src) in [
        T1306,
        "#!/bin/bash\necho {{ v }} $x\n",
        "#!/bin/bash\necho '{{ $v }}' {% if %}\n",
        "#!/bin/sh\n{# c #}\nx={{ a }}\n",
        "#!/bin/bash\nf() { echo \"${#1}\"; }\n{{ unterminated\n",
    ]
    .iter()
    .enumerate()
    {
        let name = format!("p{i}.sh");
        std::fs::write(d.join(&name), src).unwrap();
        let (_, ours) = run(&["--norc", "-f", "json1", &name], &d);
        let theirs = Command::new(oracle).args(["-f", "json1", "--norc", &name]).current_dir(&d).output().unwrap();
        assert_eq!(ours, String::from_utf8_lossy(&theirs.stdout), "{src:?}");
    }
}

#[test]
fn jinja_raw_block() {
    let d = dir("raw");
    // Raw content is shell as written: `$y` inside is still checked.
    std::fs::write(d.join("x.sh.j2"), "#!/bin/bash\n{% raw %}\necho $y\n{% endraw %}\necho {{ a }}\n").unwrap();
    let (code, out) = run(&["--norc", "-f", "gcc", "x.sh.j2"], &d);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("x.sh.j2:3:6") && out.contains("SC2086"), "{out}");
    assert!(!out.contains("SC1"), "{out}");
}

#[test]
fn jinja_unterminated_markup() {
    let d = dir("unterm");
    std::fs::write(d.join("x.sh.j2"), "#!/bin/bash\necho ok\necho {{ never closed\n$y\n").unwrap();
    // Blanked to the end of the file: nothing after it is checked, and no syntax error.
    assert_eq!(run(&["--norc", "-f", "gcc", "x.sh.j2"], &d), (0, String::new()));
}

#[test]
fn jinja_stdin_filename() {
    use std::io::Write;
    use std::process::Stdio;
    let d = dir("stdin");
    let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "gcc", "--stdin-filename", "x.sh.j2", "-"])
        .current_dir(&d)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(T1306.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert_eq!((o.status.code(), String::from_utf8_lossy(&o.stdout).into_owned()), (Some(0), String::new()));
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
fn jinja_columns_after_multibyte() {
    let d = dir("mb");
    let src = "#!/bin/bash\necho {{ caf\u{e9} }} $y\n";
    std::fs::write(d.join("x.sh.j2"), src).unwrap();
    let (_, out) = run(&["--norc", "-f", "gcc", "x.sh.j2"], &d);
    let col: usize = out.lines().find(|l| l.contains("SC2086")).unwrap().split(':').nth(2).unwrap().parse().unwrap();
    let want = src.lines().nth(1).unwrap().chars().position(|c| c == '$').unwrap() + 1;
    assert_eq!(col, want, "{out}");
}

#[test]
fn fix_respects_rc_template() {
    let d = dir("fix");
    let src = "#!/bin/bash\na={{ v }}\necho $b\n";
    std::fs::write(d.join("x.sh"), src).unwrap();
    std::fs::write(d.join(".shellcheckrc"), "template=jinja\n").unwrap();
    run(&["--fix", "x.sh"], &d);
    assert_eq!(std::fs::read_to_string(d.join("x.sh")).unwrap(), "#!/bin/bash\na={{ v }}\necho \"$b\"\n");
}

#[test]
fn fix_never_rewrites_markup() {
    let d = dir("fixmk");
    // The fix quotes the word around the markup; the markup itself stays as written.
    std::fs::write(d.join("x.sh.j2"), "#!/bin/bash\necho $x{{ v }}\n").unwrap();
    run(&["--fix", "x.sh.j2"], &d);
    let out = std::fs::read_to_string(d.join("x.sh.j2")).unwrap();
    assert_eq!(out, "#!/bin/bash\necho \"$x{{ v }}\"\n");
    assert!(!out.contains("__"));
}

#[test]
fn jinja_sourced_template_is_masked() {
    let d = dir("src");
    std::fs::write(d.join("lib.sh.j2"), "x={{ a }}\n{% if b %}\ny=1\n{% endif %}\n").unwrap();
    std::fs::write(d.join("main.sh"), "#!/bin/bash\n# shellcheck source=lib.sh.j2\nsource lib.sh.j2\n").unwrap();
    let (_, out) = run(&["--norc", "-f", "gcc", "-x", "main.sh"], &d);
    assert!(!out.contains("lib.sh.j2"), "{out}");
    assert!(!out.contains("SC1"), "{out}");
}

#[test]
fn jinja_directive_quoted() {
    let d = dir("dirq");
    for (i, q) in ["\"jinja\"", "'jinja'"].iter().enumerate() {
        let name = format!("q{i}.sh");
        std::fs::write(d.join(&name), format!("#!/bin/bash\n# shellcheck template={q}\necho {{{{ v }}}}\n")).unwrap();
        assert_eq!(run(&["--norc", "-f", "gcc", &name], &d), (0, String::new()), "{q}");
    }
}

#[test]
fn strict_rc_rejects_bad_template() {
    let d = dir("strict");
    std::fs::write(d.join("x.sh"), "#!/bin/sh\ntrue\n").unwrap();
    std::fs::write(d.join(".shellcheckrc"), "template=mako\n").unwrap();
    assert_eq!(run(&["--strict-rc", "x.sh"], &d).0, 2);
    std::fs::write(d.join(".shellcheckrc"), "template=\"jinja\"\n").unwrap();
    assert_eq!(run(&["--strict-rc", "x.sh"], &d).0, 0);
}

#[test]
fn enable_cannot_turn_on_template_mode() {
    let d = dir("enable");
    std::fs::write(d.join("x.sh"), "#!/bin/bash\necho {{ v }}\n").unwrap();
    for name in ["template-jinja", "template=jinja", "\0template-jinja"] {
        if name.contains('\0') {
            continue; // not passable in an argument
        }
        let (code, _) = run(&["--norc", &format!("--enable={name}"), "-f", "gcc", "x.sh"], &d);
        assert_eq!(code, 1, "{name}");
    }
    std::fs::write(d.join(".shellcheckrc"), "enable=template-jinja\n").unwrap();
    assert_eq!(run(&["-f", "gcc", "x.sh"], &d).0, 1);
}

#[test]
fn template_name_infers_shell_from_inner_name() {
    let d = dir("infer");
    let body = "echo hi\n";
    for n in ["x.sh", "x.sh.j2", "x.bash", "x.bash.j2"] {
        std::fs::write(d.join(n), body).unwrap();
    }
    let codes = |n: &str| {
        let (_, out) = run(&["--norc", "-f", "gcc", n], &d);
        out.lines().filter_map(|l| l.split("SC").nth(1).map(|c| c[..4].to_string())).collect::<Vec<_>>()
    };
    assert_eq!(codes("x.sh.j2"), codes("x.sh"));
    assert_eq!(codes("x.sh"), vec!["2148"]);
    assert_eq!(codes("x.bash.j2"), codes("x.bash"));
    assert!(codes("x.bash").is_empty());
}

#[test]
fn jinja_in_help_and_completions() {
    let d = dir("help");
    let (_, help) = run(&["--help"], &d);
    assert!(help.contains("--template"), "{help}");
    let (_, bash) = run(&["--completions=bash"], &d);
    assert!(bash.contains("--template"), "{bash}");
}
