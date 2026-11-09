// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: no upstream counterpart; `-f github` is shelllint's own formatter
// emitting GitHub Actions workflow commands (koalaman/shellcheck #1943).

use std::io::Write;

use shelllint_core::interface::{CheckResult, PositionedComment, SystemInterface};

use crate::*;

pub struct Github;

pub fn format() -> Github {
    Github
}

impl Formatter for Github {
    fn header(&mut self, _out: &mut dyn Write) {}
    fn footer(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    fn on_result(&mut self, cr: &CheckResult, sys: &dyn SystemInterface, out: &mut dyn Write) {
        for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let contents = read_or_empty(sys, source_file(&group[0]));
            for c in &make_non_virtual(group, &contents) {
                let _ = writeln!(out, "{}", format_comment(c));
            }
        }
    }
}

/// Escape the message part of a workflow command.
pub fn escape_data(s: &str) -> String {
    s.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}

/// Escape a property value of a workflow command (also `:` and `,`).
pub fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

fn format_comment(c: &PositionedComment) -> String {
    let command = match severity_text(c) {
        "error" => "error",
        "warning" => "warning",
        _ => "notice",
    };
    format!(
        "::{} file={},line={},col={},endLine={},endColumn={},title={}::{}",
        command,
        escape_property(source_file(c)),
        line_no(c),
        col_no(c),
        end_line_no(c),
        end_col_no(c),
        escape_property(&format!("SC{}", code_no(c))),
        escape_data(message_text(c))
    )
}
