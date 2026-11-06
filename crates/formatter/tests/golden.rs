// SPDX-License-Identifier: GPL-3.0-only
// Golden tests against the output of koalaman/shellcheck (e11f964) run with
// `--norc --color=never -f <fmt>` on tests/golden/in/{a,b,c}.sh.
//
// Inputs are rebuilt from the oracle's `json` output, which (unlike json1)
// keeps the parser's virtual tab-8 columns, i.e. what a formatter receives.

use shellcheck_core::interface::*;
use shellcheck_formatter::by_name;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{DIR}/{path}")).unwrap()
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
                    severity: match c["level"].as_str().unwrap() {
                        "error" => Severity::ErrorC,
                        "warning" => Severity::WarningC,
                        "info" => Severity::InfoC,
                        _ => Severity::StyleC,
                    },
                    code: c["code"].as_i64().unwrap(),
                    message: c["message"].as_str().unwrap().to_string(),
                },
                fix,
            }
        })
        .collect();
    (CheckResult { filename: name.clone(), comments }, read(&format!("in/{name}")))
}

fn run(fmt: &str, script: &str, color: ColorOption) -> (String, bool) {
    let mut opts = new_formatter_options();
    opts.color_option = color;
    let mut f = by_name(fmt, opts).unwrap();
    let (cr, src) = result_for(script);
    let mut out: Vec<u8> = vec![];
    f.header(&mut out);
    f.on_result(&cr, &src, &mut out);
    f.footer(&mut out);
    (String::from_utf8(out).unwrap(), f.exit_requested())
}

fn check(fmt: &str) {
    for s in ["a", "b", "c"] {
        let (got, _) = run(fmt, s, ColorOption::ColorNever);
        assert_eq!(got, read(&format!("{s}.{fmt}.txt")), "{fmt} {s}");
    }
}

#[test]
fn golden_tty() {
    check("tty");
}
#[test]
fn golden_gcc() {
    check("gcc");
}
#[test]
fn golden_json() {
    check("json");
}
#[test]
fn golden_json1() {
    check("json1");
}
#[test]
fn golden_checkstyle() {
    check("checkstyle");
}
#[test]
fn golden_quiet() {
    for s in ["a", "b", "c"] {
        let (got, exit) = run("quiet", s, ColorOption::ColorNever);
        assert_eq!(got, read(&format!("{s}.quiet.txt")));
        assert_eq!(exit, s != "a");
    }
}

#[test]
fn golden_tty_color_always() {
    let (got, _) = run("tty", "b", ColorOption::ColorAlways);
    assert_eq!(got, read("b.tty-color.txt"));
}

#[test]
fn json_escaping_matches_aeson() {
    let mut f = by_name("json1", new_formatter_options()).unwrap();
    let file = "x\"\\\u{1}\u{7f}\u{8}\u{c}\u{e9}\u{20ac}\u{1f600}.sh";
    let p = Position { file: file.into(), line: 1, column: 1 };
    let cr = CheckResult {
        filename: file.into(),
        comments: vec![PositionedComment {
            start_pos: p.clone(),
            end_pos: p,
            comment: Comment { severity: Severity::ErrorC, code: 1, message: "m".into() },
            fix: None,
        }],
    };
    let mut out = vec![];
    f.on_result(&cr, "", &mut out);
    f.footer(&mut out);
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("\"file\":\"x\\\"\\\\\\u0001\u{7f}\\u0008\\u000c\u{e9}\u{20ac}\u{1f600}.sh\""), "{s}");
}

#[test]
fn unknown_formatter() {
    assert!(by_name("nope", new_formatter_options()).is_none());
}
