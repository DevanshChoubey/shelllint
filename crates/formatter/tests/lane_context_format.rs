// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for the `-f context` formatter (an extension beyond ShellCheck).

use shelllint_core::interface::*;
use shelllint_formatter::by_name;

fn finding(line: i64, col: i64, sev: Severity, code: i64, msg: &str) -> PositionedComment {
    let pos = |column| Position { file: "t.sh".to_string(), line, column };
    PositionedComment {
        start_pos: pos(col),
        end_pos: pos(col + 1),
        comment: Comment { severity: sev, code, message: msg.to_string() },
        fix: None,
    }
}

fn run(src: &str, comments: Vec<PositionedComment>, context: Option<i64>) -> String {
    let mut opts = new_formatter_options();
    if let Some(n) = context {
        opts.context_lines = n;
    }
    let mut f = by_name("context", opts).unwrap();
    let cr = CheckResult { filename: "t.sh".to_string(), comments };
    let sys = mocked_system_interface(vec![("t.sh".to_string(), src.to_string())]);
    let mut out: Vec<u8> = vec![];
    f.header(&mut out);
    f.on_result(&cr, &sys, &mut out);
    f.footer(&mut out);
    String::from_utf8(out).unwrap()
}

const SCRIPT: &str = "#!/bin/sh\nx=1\necho $x\nexit 0\n";

#[test]
fn exact_output_small_script() {
    let got = run(SCRIPT, vec![finding(3, 6, Severity::InfoC, 2086, "Double quote to prevent globbing.")], Some(1));
    let want = "t.sh:3:6: info: Double quote to prevent globbing. [SC2086]\n\
                2 | x=1\n\
                3 | echo $x\n\
                \x20 |      ^\n\
                4 | exit 0\n";
    assert_eq!(got, want);
}

#[test]
fn default_is_five_lines_and_zero_is_allowed() {
    let src: String = (1..=20).map(|i| format!("l{i}\n")).collect();
    let f = || finding(10, 1, Severity::WarningC, 1, "m");
    let got = run(&src, vec![f()], None);
    let nums: Vec<&str> = got.lines().skip(1).filter(|l| !l.trim_start().starts_with('|')).collect();
    assert_eq!(nums.first(), Some(&" 5 | l5"));
    assert_eq!(nums.last(), Some(&"15 | l15"));
    assert_eq!(nums.len(), 11);
    assert_eq!(run(&src, vec![f()], Some(0)), "t.sh:10:1: warning: m [SC1]\n10 | l10\n   | ^\n");
}

#[test]
fn clamps_at_file_start_and_end() {
    let start = run(SCRIPT, vec![finding(1, 1, Severity::ErrorC, 1008, "m")], Some(10));
    assert_eq!(start, "t.sh:1:1: error: m [SC1008]\n1 | #!/bin/sh\n  | ^\n2 | x=1\n3 | echo $x\n4 | exit 0\n");
    let end = run(SCRIPT, vec![finding(4, 8, Severity::StyleC, 9, "m")], Some(2));
    assert_eq!(end, "t.sh:4:8: style: m [SC9]\n2 | x=1\n3 | echo $x\n4 | exit 0\n  |        ^\n");
}

#[test]
fn multiple_findings_are_separated_by_a_blank_line() {
    let got =
        run(SCRIPT, vec![finding(2, 1, Severity::WarningC, 1, "a"), finding(4, 1, Severity::InfoC, 2, "b")], Some(0));
    let want = "t.sh:2:1: warning: a [SC1]\n2 | x=1\n  | ^\n\nt.sh:4:1: info: b [SC2]\n4 | exit 0\n  | ^\n";
    assert_eq!(got, want);
}

#[test]
fn no_findings_prints_nothing() {
    assert_eq!(run(SCRIPT, vec![], None), "");
}
