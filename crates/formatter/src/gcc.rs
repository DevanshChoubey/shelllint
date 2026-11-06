// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/GCC.hs (e11f964)

use std::io::Write;

use shellcheck_core::interface::{CheckResult, PositionedComment, SystemInterface};

use crate::*;

pub struct Gcc;

pub fn format() -> Gcc {
    Gcc
}

impl Formatter for Gcc {
    fn header(&mut self, _out: &mut dyn Write) {}
    fn footer(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    fn on_result(&mut self, cr: &CheckResult, sys: &dyn SystemInterface, out: &mut dyn Write) {
        for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let contents = read_or_empty(sys, source_file(&group[0]));
            output_result(&contents, group, out);
        }
    }
}

fn output_result(contents: &str, warnings: &[PositionedComment], out: &mut dyn Write) {
    let comments = make_non_virtual(warnings, contents);
    for c in &comments {
        let _ = writeln!(out, "{}", format_comment(&source_file(c).to_string(), c));
    }
}

fn format_comment(filename: &str, c: &PositionedComment) -> String {
    let level = match severity_text(c) {
        "error" => "error",
        "warning" => "warning",
        _ => "note",
    };
    format!(
        "{}:{}:{}: {}: {} [SC{}]",
        filename,
        line_no(c),
        col_no(c),
        level,
        haskell_lines(message_text(c)).concat(),
        code_no(c)
    )
}
