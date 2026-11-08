// SPDX-License-Identifier: GPL-3.0-or-later
// Oracle tests for the `diff` formatter against koalaman/shellcheck (e11f964).
//
// Fixtures live in tests/golden/diff/. For each `<name>.sh`:
//   <name>.json.txt  = docker run --rm -i -v $PWD:/mnt -w /mnt koalaman/shellcheck:latest -f json --norc <name>.sh
//   <name>.diff.txt  = docker run --rm -i -v $PWD:/mnt -w /mnt koalaman/shellcheck:latest -f diff --norc <name>.sh
// twofiles.diff.txt  = same image, `-f diff --norc -x main.sh lib.sh`
//
// The json output is turned back into a CheckResult (as in golden.rs), so
// these tests exercise only the formatter, not the (partially ported) checks.
//
// Note: `-x` does not make shellcheck report issues inside sourced files, so
// real cross-file fixes cannot be produced by the oracle; that path is covered
// by the synthetic `cross_file_fix_uses_reader` test instead.

use shelllint_core::interface::*;
use shelllint_formatter::Formatter;
use shelllint_formatter::diff;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/diff");

fn read(path: &str) -> String {
    std::fs::read(format!("{DIR}/{path}")).map(|b| String::from_utf8(b).unwrap()).unwrap()
}

fn pos(file: &str, line: &serde_json::Value, col: &serde_json::Value) -> Position {
    Position { file: file.to_string(), line: line.as_i64().unwrap(), column: col.as_i64().unwrap() }
}

fn result_for(script: &str) -> (CheckResult, String) {
    let name = format!("{script}.sh");
    let v: serde_json::Value = serde_json::from_str(&read(&format!("{script}.json.txt"))).unwrap();
    let comments = v
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let file = c["file"].as_str().unwrap();
            let fix = if c["fix"].is_null() {
                None
            } else {
                Some(Fix {
                    replacements: c["fix"]["replacements"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| Replacement {
                            start_pos: pos(file, &r["line"], &r["column"]),
                            end_pos: pos(file, &r["endLine"], &r["endColumn"]),
                            string: r["replacement"].as_str().unwrap().to_string(),
                            precedence: r["precedence"].as_i64().unwrap(),
                            insertion_point: if r["insertionPoint"] == "beforeStart" {
                                InsertionPoint::InsertBefore
                            } else {
                                InsertionPoint::InsertAfter
                            },
                        })
                        .collect(),
                })
            };
            PositionedComment {
                start_pos: pos(file, &c["line"], &c["column"]),
                end_pos: pos(file, &c["endLine"], &c["endColumn"]),
                comment: Comment {
                    severity: Severity::StyleC,
                    code: c["code"].as_i64().unwrap(),
                    message: c["message"].as_str().unwrap().to_string(),
                },
                fix,
            }
        })
        .collect();
    (CheckResult { filename: name.clone(), comments }, read(&name))
}

fn run_many(scripts: &[&str], color: ColorOption) -> String {
    let mut opts = new_formatter_options();
    opts.color_option = color;
    let mut f = diff::format(opts);
    let mut out: Vec<u8> = vec![];
    f.header(&mut out);
    for s in scripts {
        let (cr, src) = result_for(s);
        let sys = mocked_system_interface(vec![(cr.filename.clone(), src)]);
        f.on_result(&cr, &sys, &mut out);
    }
    f.footer(&mut out);
    String::from_utf8(out).unwrap()
}

fn check(script: &str) {
    assert_eq!(run_many(&[script], ColorOption::ColorNever), read(&format!("{script}.diff.txt")), "{script}");
}

#[test]
fn oracle_echo() {
    check("echo");
}
#[test]
fn oracle_double_bracket() {
    check("dblbracket");
}
#[test]
fn oracle_backtick() {
    check("backtick");
}
#[test]
fn oracle_multiple_fixes() {
    check("multi");
}
#[test]
fn oracle_crlf() {
    check("crlf");
}
#[test]
fn oracle_no_trailing_linefeed() {
    check("nolf");
}
#[test]
fn oracle_tabs() {
    check("tab");
}
#[test]
fn oracle_no_fixes_prints_nothing() {
    check("nofix");
}
#[test]
fn oracle_two_files() {
    assert_eq!(run_many(&["main", "lib"], ColorOption::ColorNever), read("twofiles.diff.txt"));
}

#[test]
fn color_always_wraps_headers_and_lines() {
    let got = run_many(&["echo"], ColorOption::ColorAlways);
    let want = "\x1B[1m--- a/echo.sh\x1B[0m\n\x1B[1m+++ b/echo.sh\x1B[0m\n\
        \x1B[36m@@ -1,1 +1,1 @@\x1B[0m\n\x1B[31m-echo $1\x1B[0m\n\x1B[32m+echo \"$1\"\x1B[0m\n\n";
    assert_eq!(got, want);
}

#[test]
fn cross_file_fix_reads_each_file() {
    let p = |f: &str, c| Position { file: f.into(), line: 1, column: c };
    let rep = |f: &str, c, s: &str, ins| Replacement {
        start_pos: p(f, c),
        end_pos: p(f, c),
        string: s.into(),
        precedence: 1,
        insertion_point: ins,
    };
    let fix = Fix {
        replacements: vec![
            rep("main.sh", 1, "# ", InsertionPoint::InsertBefore),
            rep("other.sh", 1, "# ", InsertionPoint::InsertBefore),
        ],
    };
    let cr = CheckResult {
        filename: "main.sh".into(),
        comments: vec![PositionedComment {
            start_pos: p("main.sh", 1),
            end_pos: p("main.sh", 1),
            comment: Comment { severity: Severity::StyleC, code: 1, message: "m".into() },
            fix: Some(fix),
        }],
    };
    let mut f = diff::format(new_formatter_options());
    let sys = mocked_system_interface(vec![("main.sh".into(), "echo\n".into()), ("other.sh".into(), "ls\n".into())]);
    let mut out = vec![];
    f.on_result(&cr, &sys, &mut out);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "--- a/main.sh\n+++ b/main.sh\n@@ -1,1 +1,1 @@\n-echo\n+# echo\n\n\
         --- a/other.sh\n+++ b/other.sh\n@@ -1,1 +1,1 @@\n-ls\n+# ls\n\n"
    );
}
