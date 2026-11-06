// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Formatter/CheckStyle.hs (e11f964)

use std::io::Write;

use shellcheck_core::interface::{CheckResult, PositionedComment};

use crate::*;

pub struct CheckStyle;

pub fn format() -> CheckStyle {
    CheckStyle
}

impl Formatter for CheckStyle {
    fn header(&mut self, out: &mut dyn Write) {
        let _ = writeln!(out, "<?xml version='1.0' encoding='UTF-8'?>");
        let _ = writeln!(out, "<checkstyle version='4.3'>");
    }

    fn on_failure(&mut self, file: &str, error: &str, out: &mut dyn Write) {
        let _ = writeln!(
            out,
            "<file {}>\n<error {}{}{}{}{}/>\n</file>",
            attr("name", file),
            attr("line", "1"),
            attr("column", "1"),
            attr("severity", "error"),
            attr("message", error),
            attr("source", "ShellCheck"),
        );
    }

    fn on_result(&mut self, cr: &CheckResult, source: &str, out: &mut dyn Write) {
        if cr.comments.is_empty() {
            output_file(&cr.filename, "", &[], out);
        } else {
            for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
                output_file(source_file(&group[0]), source, group, out);
            }
        }
    }

    fn footer(&mut self, out: &mut dyn Write) {
        let _ = writeln!(out, "</checkstyle>");
    }
}

fn output_file(filename: &str, contents: &str, warnings: &[PositionedComment], out: &mut dyn Write) {
    let comments = make_non_virtual(warnings, contents);
    let _ = writeln!(out, "{}", format_file(filename, &comments));
}

fn format_file(name: &str, comments: &[PositionedComment]) -> String {
    let mut s = format!("<file {}>\n", attr("name", name));
    for c in comments {
        s.push_str(&format_comment(c));
    }
    s.push_str("</file>");
    s
}

fn format_comment(c: &PositionedComment) -> String {
    format!(
        "<error {}{}{}{}{}/>\n",
        attr("line", &line_no(c).to_string()),
        attr("column", &col_no(c).to_string()),
        attr("severity", severity(severity_text(c))),
        attr("message", message_text(c)),
        attr("source", &format!("ShellCheck.SC{}", code_no(c))),
    )
}

fn attr(s: &str, v: &str) -> String {
    format!("{}='{}' ", s, escape(v))
}

fn escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if is_ok(c) {
                c.to_string()
            } else {
                format!("&#{};", c as u32)
            }
        })
        .collect()
}

fn is_ok(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_lowercase() || c.is_ascii_digit() || " ./".contains(c)
}

fn severity(s: &str) -> &'static str {
    match s {
        "error" => "error",
        "warning" => "warning",
        _ => "info",
    }
}
