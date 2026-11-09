// SPDX-License-Identifier: GPL-3.0-or-later
use shelllint_core::interface::*;
use shelllint_formatter::{by_name, github};

fn pc(file: &str, sev: Severity, code: i64, msg: &str) -> PositionedComment {
    let p = |l, c| Position { file: file.to_string(), line: l, column: c };
    PositionedComment {
        start_pos: p(3, 5),
        end_pos: p(3, 9),
        comment: Comment { severity: sev, code, message: msg.to_string() },
        fix: None,
    }
}

fn run(comments: Vec<PositionedComment>) -> String {
    let mut f = by_name("github", new_formatter_options()).unwrap();
    let cr = CheckResult { filename: "x.sh".into(), comments };
    let sys = mocked_system_interface(vec![]);
    let mut out: Vec<u8> = vec![];
    f.header(&mut out);
    f.on_result(&cr, &sys, &mut out);
    f.footer(&mut out);
    String::from_utf8(out).unwrap()
}

#[test]
fn levels_and_exact_output() {
    let got = run(vec![
        pc("x.sh", Severity::ErrorC, 1000, "e"),
        pc("x.sh", Severity::WarningC, 2086, "Double quote to prevent globbing."),
        pc("x.sh", Severity::InfoC, 2001, "i"),
        pc("x.sh", Severity::StyleC, 2006, "s"),
    ]);
    let want = "\
::error file=x.sh,line=3,col=5,endLine=3,endColumn=9,title=SC1000::e
::warning file=x.sh,line=3,col=5,endLine=3,endColumn=9,title=SC2086::Double quote to prevent globbing.
::notice file=x.sh,line=3,col=5,endLine=3,endColumn=9,title=SC2001::i
::notice file=x.sh,line=3,col=5,endLine=3,endColumn=9,title=SC2006::s
";
    assert_eq!(got, want);
}

#[test]
fn escaping() {
    let got = run(vec![pc("a:b,c%d.sh", Severity::ErrorC, 1, "50% a:b, c\r\nnext")]);
    assert_eq!(
        got,
        "::error file=a%3Ab%2Cc%25d.sh,line=3,col=5,endLine=3,endColumn=9,title=SC1::50%25 a:b, c%0D%0Anext\n"
    );
}

#[test]
fn escape_helpers() {
    assert_eq!(github::escape_data("a:b,c%\n"), "a:b,c%25%0A");
    assert_eq!(github::escape_property("a:b,c%\r"), "a%3Ab%2Cc%25%0D");
}

#[test]
fn empty_output_without_comments() {
    assert_eq!(run(vec![]), "");
}
