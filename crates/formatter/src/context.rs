// SPDX-License-Identifier: GPL-3.0-or-later
//
// DIVERGENCE: `-f context` is an extension; ShellCheck has no such format.
//
// For each finding: a gcc-style header line, then the source lines around it
// (`--context=N` on each side, clamped to the file) with line numbers and a
// caret under the reported column. Plain text, never coloured.

use std::io::Write;

use shelllint_core::interface::{CheckResult, FormatterOptions, PositionedComment, SystemInterface};

use crate::*;

pub struct Context {
    context: usize,
    wrote_any: bool,
}

pub fn format(options: FormatterOptions) -> Context {
    Context { context: options.context_lines.max(0) as usize, wrote_any: false }
}

impl Formatter for Context {
    fn header(&mut self, _out: &mut dyn Write) {}
    fn footer(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    fn on_result(&mut self, cr: &CheckResult, sys: &dyn SystemInterface, out: &mut dyn Write) {
        for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let contents = read_or_empty(sys, source_file(&group[0]));
            let comments = make_non_virtual(group, &contents);
            let lines = haskell_lines(&contents);
            for c in &comments {
                if self.wrote_any {
                    let _ = writeln!(out);
                }
                self.wrote_any = true;
                output_finding(c, &lines, self.context, out);
            }
        }
    }
}

fn output_finding(c: &PositionedComment, lines: &[&str], context: usize, out: &mut dyn Write) {
    let _ = writeln!(
        out,
        "{}:{}:{}: {}: {} [SC{}]",
        source_file(c),
        line_no(c),
        col_no(c),
        severity_text(c),
        haskell_lines(message_text(c)).concat(),
        code_no(c)
    );
    if lines.is_empty() {
        return;
    }
    let line = line_no(c).clamp(1, lines.len() as i64) as usize;
    let first = line.saturating_sub(context).max(1);
    let last = line.saturating_add(context).min(lines.len());
    let width = last.to_string().len();
    for n in first..=last {
        let text = lines[n - 1];
        if text.is_empty() {
            let _ = writeln!(out, "{:>w$} |", n, w = width);
        } else {
            let _ = writeln!(out, "{:>w$} | {}", n, text, w = width);
        }
        if n as i64 == line_no(c) {
            let _ = writeln!(out, "{:w$} | {}^", "", caret_padding(text, col_no(c)), w = width);
        }
    }
}

/// Blanks up to (not including) column `col`, keeping tabs so the caret lines
/// up with the source line above it.
fn caret_padding(text: &str, col: i64) -> String {
    let n = (col - 1).max(0) as usize;
    let mut chars = text.chars();
    (0..n).map(|_| if chars.next() == Some('\t') { '\t' } else { ' ' }).collect()
}
