// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--list-checks[=text|json]` (extension; see src/list_checks.rs).

use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint")).args(args).env_remove("SHELLCHECK_OPTS").output().unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// Minimal JSON well-formedness check (no JSON crate here): brackets balance
/// outside strings, strings terminate, escapes are valid.
fn assert_json_wellformed(s: &str) {
    let mut stack = vec![];
    let mut chars = s.trim_end().chars();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            match c {
                '\\' => assert!("\"\\/bfnrtu".contains(chars.next().unwrap())),
                '"' => in_str = false,
                c => assert!(c as u32 >= 0x20, "raw control char"),
            }
        } else {
            match c {
                '"' => in_str = true,
                '{' | '[' => stack.push(c),
                '}' => assert_eq!(stack.pop(), Some('{')),
                ']' => assert_eq!(stack.pop(), Some('[')),
                _ => {}
            }
        }
    }
    assert!(!in_str && stack.is_empty());
}

#[test]
fn text_lists_a_known_optional_check() {
    for args in [&["--list-checks"][..], &["--list-checks=text"]] {
        let (code, out, _) = run(args);
        assert_eq!(code, 0);
        let line = out.lines().find(|l| l.starts_with("require-variable-braces")).unwrap();
        assert!(line.contains("SC2250"), "{}", line);
        assert!(out.contains("no catalogue"), "missing-catalogue note");
    }
}

#[test]
fn json_parses_and_has_known_check() {
    let (code, out, _) = run(&["--list-checks=json"]);
    assert_eq!(code, 0);
    assert_json_wellformed(&out);
    assert!(out.starts_with("{\"optional_checks\":["));
    assert!(out.contains("\"name\":\"require-variable-braces\""));
    assert!(out.contains("\"all_codes\":null"));
}

#[test]
fn json_has_every_optional_check() {
    let (_, listed, _) = run(&["--list-optional"]);
    let (_, json, _) = run(&["--list-checks=json"]);
    for name in listed.lines().filter_map(|l| l.strip_prefix("name:    ")) {
        assert!(json.contains(&format!("\"name\":\"{}\"", name)), "{}", name);
    }
}

#[test]
fn unknown_format_fails() {
    let (code, out, err) = run(&["--list-checks=yaml"]);
    assert_eq!(code, 4);
    assert!(out.is_empty());
    assert!(err.contains("Unknown value for --list-checks"));
}
