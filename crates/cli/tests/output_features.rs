// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the output extensions: `-f sarif`, `--stdin-filename` and
// `--output=FORMAT:PATH`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_in(dir: Option<&Path>, args: &[&str], stdin: &str) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shelllint"));
    cmd.args(args)
        .env_remove("SHELLCHECK_OPTS")
        .env("XDG_CONFIG_HOME", "/nonexistent")
        .env("HOME", "/nonexistent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let mut child = cmd.spawn().unwrap();
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let o = child.wait_with_output().unwrap();
    Out {
        code: o.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn run(args: &[&str], stdin: &str) -> Out {
    run_in(None, args, stdin)
}

static N: AtomicUsize = AtomicUsize::new(0);

fn tmpdir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "shelllint-outfeat-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(d).unwrap()
}

fn write(p: &Path, c: &str) -> String {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, c).unwrap();
    p.to_string_lossy().into_owned()
}

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("invalid JSON ({}): {}", e, s))
}

/// SC2086 (info) on line 2 and SC2154 (warning) on line 2.
const WARN: &str = "#!/bin/sh\necho $1 $undefined_var\n";
const CLEAN: &str = "#!/bin/sh\ntrue\n";

// ---- -f sarif ----

#[test]
fn sarif_is_valid_sarif_json() {
    let d = tmpdir();
    let f = write(&d.join("dir with space/ü.sh"), WARN);
    let o = run_in(Some(&d), &["-f", "sarif", "dir with space/ü.sh"], "");
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stdout.ends_with('\n'));
    let v = json(&o.stdout);
    assert_eq!(v["$schema"], "https://json.schemastore.org/sarif-2.1.0.json");
    assert_eq!(v["version"], "2.1.0");
    let runs = v["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    let driver = &runs[0]["tool"]["driver"];
    assert_eq!(driver["name"], "shelllint");
    assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(driver["informationUri"], "https://github.com/DevanshChoubey/shelllint");

    // Same findings and positions as json1.
    let j1 = json(&run_in(Some(&d), &["-f", "json1", "dir with space/ü.sh"], "").stdout);
    let mut expected: Vec<(i64, String, i64, i64, i64, i64)> = j1["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["code"].as_i64().unwrap(),
                c["message"].as_str().unwrap().to_string(),
                c["line"].as_i64().unwrap(),
                c["column"].as_i64().unwrap(),
                c["endLine"].as_i64().unwrap(),
                c["endColumn"].as_i64().unwrap(),
            )
        })
        .collect();
    assert!(expected.len() >= 2, "{:?}", expected);
    let results = runs[0]["results"].as_array().unwrap();
    let mut got = vec![];
    for r in results {
        let id = r["ruleId"].as_str().unwrap();
        let loc = &r["locations"].as_array().unwrap()[0]["physicalLocation"];
        // Relative paths stay relative; spaces and non-ASCII are percent-encoded.
        assert_eq!(loc["artifactLocation"]["uri"], "dir%20with%20space/%C3%BC.sh");
        let reg = &loc["region"];
        got.push((
            id.trim_start_matches("SC").parse::<i64>().unwrap(),
            r["message"]["text"].as_str().unwrap().to_string(),
            reg["startLine"].as_i64().unwrap(),
            reg["startColumn"].as_i64().unwrap(),
            reg["endLine"].as_i64().unwrap(),
            reg["endColumn"].as_i64().unwrap(),
        ));
        let sev =
            j1["comments"].as_array().unwrap().iter().find(|c| format!("SC{}", c["code"]) == id).unwrap()["level"]
                .as_str()
                .unwrap();
        let want = if sev == "error" || sev == "warning" { sev } else { "note" };
        assert_eq!(r["level"], want);
        let idx = r["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(runs[0]["tool"]["driver"]["rules"][idx]["id"], id);
    }
    expected.sort();
    got.sort();
    assert_eq!(got, expected);

    // One rule per distinct code, with its wiki page.
    let rules = driver["rules"].as_array().unwrap();
    let mut ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert!(ids.contains(&"SC2086"));
    for r in rules {
        assert_eq!(r["helpUri"], format!("https://www.shellcheck.net/wiki/{}", r["id"].as_str().unwrap()));
    }
    let _ = f;
}

#[test]
fn sarif_with_no_findings_is_still_a_document() {
    let d = tmpdir();
    let f = write(&d.join("ok.sh"), CLEAN);
    let o = run(&["-f", "sarif", &f], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v = json(&o.stdout);
    assert_eq!(v["version"], "2.1.0");
    assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
    assert_eq!(v["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap().len(), 0);
}

#[test]
fn sarif_is_one_document_for_all_files() {
    let d = tmpdir();
    let a = write(&d.join("a.sh"), WARN);
    let b = write(&d.join("b.sh"), WARN);
    let o = run(&["-f", "sarif", &a, &b], "");
    assert_eq!(o.code, 1);
    let v = json(&o.stdout);
    assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    let uris: std::collections::BTreeSet<String> = v["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(uris.len(), 2, "{:?}", uris);
    // Absolute paths become file:// URIs.
    assert!(uris.iter().all(|u| u.starts_with("file:///")), "{:?}", uris);
    // The rule list has one entry per code, however many results use it.
    let rules = v["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap();
    let results = v["runs"][0]["results"].as_array().unwrap();
    assert!(results.len() > rules.len());
}

#[test]
fn sarif_is_listed_in_help_and_format_errors() {
    let o = run(&["--help"], "");
    assert!(o.stdout.contains("json1, quiet, sarif, tty)"), "{}", o.stdout);
    let o = run(&["-f", "nope", "-"], "");
    assert_eq!(o.code, 4);
    assert!(o.stderr.contains("  sarif\n"), "{}", o.stderr);
}

// ---- --stdin-filename ----

#[test]
fn stdin_filename_names_the_script_in_every_format() {
    for (fmt, needle) in [
        ("gcc", "some/where.sh:2:"),
        ("tty", "In some/where.sh line 2:"),
        ("json1", "\"file\":\"some/where.sh\""),
        ("json", "\"file\":\"some/where.sh\""),
        ("checkstyle", "<file name='some/where.sh' >"),
        ("sarif", "\"uri\":\"some/where.sh\""),
    ] {
        let o = run(&["-f", fmt, "--stdin-filename=some/where.sh", "-"], WARN);
        assert_eq!(o.code, 1, "{}: {}", fmt, o.stderr);
        assert!(o.stdout.contains(needle), "{}: {}", fmt, o.stdout);
        assert!(
            !o.stdout.contains("'-'") && !o.stdout.contains("\"-\"") && !o.stdout.contains("- line"),
            "{}: {}",
            fmt,
            o.stdout
        );
    }
    // The diff format also uses the name.
    let o = run(&["-f", "diff", "--stdin-filename=some/where.sh", "-"], "#!/bin/sh\necho `date`\n");
    assert!(o.stdout.contains("--- a/some/where.sh"), "{}", o.stdout);
    // Space-separated form and without the option: unchanged.
    let o = run(&["-f", "gcc", "--stdin-filename", "x.sh", "-"], WARN);
    assert!(o.stdout.starts_with("x.sh:2:"), "{}", o.stdout);
    let o = run(&["-f", "gcc", "-"], WARN);
    assert!(o.stdout.starts_with("-:2:"), "{}", o.stdout);
}

#[test]
fn stdin_filename_resolves_source_relative_to_its_directory() {
    let d = tmpdir();
    write(&d.join("proj/lib.sh"), "foo=1\n");
    let script = "#!/bin/sh\n# shellcheck source-path=SCRIPTDIR\n. lib.sh\necho \"$foo\"\n";
    let main = d.join("proj/main.sh");
    let main = main.to_string_lossy().into_owned();
    let other = tmpdir(); // cwd that does not contain lib.sh

    // The file need not exist; its directory is what counts.
    let arg = format!("--stdin-filename={}", main);
    let o = run_in(Some(&other), &["-x", "-f", "gcc", &arg, "-"], script);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert_eq!(o.stdout, "");

    // Without it, `-` has no directory and the source cannot be found.
    let o = run_in(Some(&other), &["-x", "-f", "gcc", "-"], script);
    assert_eq!(o.code, 1);
    assert!(o.stdout.contains("SC1091"), "{}", o.stdout);

    // A relative PATH is relative to the cwd, like a real file.
    let o = run_in(Some(&d), &["-x", "-f", "gcc", "--stdin-filename=proj/main.sh", "-"], script);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
}

#[test]
fn stdin_filename_directory_supplies_shellcheckrc_and_editorconfig() {
    let d = tmpdir();
    write(&d.join("p/.shellcheckrc"), "disable=SC2086\n");
    let arg = format!("--stdin-filename={}/p/x.sh", d.display());
    let o = run(&["-f", "gcc", &arg, "-"], "#!/bin/sh\necho $1\n");
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    let o = run(&["-f", "gcc", "--norc", &arg, "-"], "#!/bin/sh\necho $1\n");
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("SC2086"));

    write(&d.join("e/.editorconfig"), "root = true\n[*.sh]\nshellcheck.disable = SC2148,SC2086\n");
    // Mixing `/` into a canonical Windows `\\?\` path would make it a different file.
    let root = d.display().to_string();
    let root = root.strip_prefix(r"\\?\").unwrap_or(&root);
    let arg = format!("--stdin-filename={}/e/x.sh", root);
    // The .editorconfig next to the (virtual) script applies.
    let o = run(&["-f", "gcc", &arg, "-"], "echo $1\n");
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    let other = format!("--stdin-filename={}/no-editorconfig/x.sh", root);
    let o = run(&["-f", "gcc", &other, "-"], "echo $1\n");
    assert!(o.stdout.contains("SC2148"), "{}", o.stdout);
}

#[test]
fn stdin_filename_needs_dash() {
    let d = tmpdir();
    let f = write(&d.join("a.sh"), CLEAN);
    let o = run(&["--stdin-filename=x.sh", &f], "");
    assert_eq!(o.code, 3);
    assert!(o.stderr.contains("--stdin-filename can only be used together with '-'"), "{}", o.stderr);
    assert_eq!(o.stdout, "");
    // The same code as another bad flag.
    assert_eq!(run(&["--bogus", &f], "").code, 3);
}

// ---- --output ----

#[test]
fn two_outputs_from_one_run() {
    let d = tmpdir();
    let j = d.join("out.json").to_string_lossy().into_owned();
    let s = d.join("out.sarif").to_string_lossy().into_owned();
    let (oj, os) = (format!("--output=json1:{}", j), format!("--output=sarif:{}", s));
    // Stdin can only be read once, so this also shows the script is checked once.
    let o = run(&["-f", "gcc", &oj, &os, "--stdin-filename=a.sh", "-"], WARN);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stdout.starts_with("a.sh:2:"), "{}", o.stdout);
    assert_eq!(o.stderr, "");

    let plain = run(&["-f", "json1", "--stdin-filename=a.sh", "-"], WARN);
    assert_eq!(std::fs::read_to_string(&j).unwrap(), plain.stdout);
    let sarif = run(&["-f", "sarif", "--stdin-filename=a.sh", "-"], WARN);
    assert_eq!(std::fs::read_to_string(&s).unwrap(), sarif.stdout);
    json(&std::fs::read_to_string(&s).unwrap());
}

#[test]
fn output_works_on_a_clean_run_and_keeps_exit_code() {
    let d = tmpdir();
    let f = write(&d.join("ok.sh"), CLEAN);
    let p = d.join("r.sarif").to_string_lossy().into_owned();
    let o = run(&[&format!("--output=sarif:{}", p), &f], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.stdout, "");
    assert_eq!(json(&std::fs::read_to_string(&p).unwrap())["version"], "2.1.0");
}

// Windows filenames can't contain a colon.
#[cfg(unix)]
#[test]
fn output_splits_at_the_first_colon() {
    let d = tmpdir();
    let f = write(&d.join("ok.sh"), CLEAN);
    let p = d.join("a:b.json");
    let o = run(&[&format!("--output=json:{}", p.display()), &f], "");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "[]\n");
}

#[test]
fn output_errors() {
    let d = tmpdir();
    let f = write(&d.join("w.sh"), WARN);
    // Unknown format: same error as -f.
    let p = d.join("x").to_string_lossy().into_owned();
    let o = run(&[&format!("--output=nope:{}", p), &f], "");
    let bad_f = run(&["-f", "nope", &f], "");
    assert_eq!(o.code, 4);
    assert_eq!(o.stderr, bad_f.stderr);
    assert!(!Path::new(&p).exists());
    // No colon, or an empty part.
    for v in ["json", ":x", "json:"] {
        let o = run(&[&format!("--output={}", v), &f], "");
        assert_eq!(o.code, 3, "{}: {}", v, o.stderr);
        assert!(o.stderr.contains("--output"), "{}", o.stderr);
    }
    // A file that cannot be created: error, exit 2, nothing on stdout.
    let bad = d.join("missing-dir/out.json").to_string_lossy().into_owned();
    let o = run(&["-f", "gcc", &format!("--output=json:{}", bad), &f], "");
    assert_eq!(o.code, 2, "{}", o.stderr);
    assert_eq!(o.stdout, "");
    assert!(o.stderr.contains("Could not write output file"), "{}", o.stderr);
    assert!(o.stderr.contains("does not exist"), "{}", o.stderr);
}

#[test]
fn output_does_not_change_the_normal_report() {
    let d = tmpdir();
    let f = write(&d.join("w.sh"), WARN);
    let p = d.join("o.txt").to_string_lossy().into_owned();
    let plain = run(&["-f", "gcc", &f], "");
    let with = run(&["-f", "gcc", &format!("--output=checkstyle:{}", p), &f], "");
    assert_eq!((plain.code, &plain.stdout, &plain.stderr), (with.code, &with.stdout, &with.stderr));
    assert!(std::fs::read_to_string(&p).unwrap().starts_with("<?xml"));
}
