// SPDX-License-Identifier: GPL-3.0-or-later
//! Running the checker on a buffer and turning its comments into LSP data.

use lsp_types::{DiagnosticSeverity, Position, Range, TextEdit};
use shellcheck_checker::check_script;
use shellcheck_core::interface::{CheckSpec, PositionedComment, Severity, empty_check_spec};

use crate::config::Settings;
use crate::position::{split_lines, to_lsp};
use crate::sys::LspSystem;

pub const WIKI_LINK: &str = "https://www.shellcheck.net/wiki/";

/// One ShellCheck comment, already in LSP coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: i64,
    pub message: String,
    /// Edits of ShellCheck's suggested fix; empty when there is none.
    pub fix: Vec<TextEdit>,
}

impl Finding {
    pub fn contains(&self, pos: Position) -> bool {
        let r = self.range;
        pos >= r.start && (pos < r.end || (r.start == r.end && pos == r.start))
    }
}

pub fn lsp_severity(s: Severity) -> DiagnosticSeverity {
    match s {
        Severity::ErrorC => DiagnosticSeverity::ERROR,
        Severity::WarningC => DiagnosticSeverity::WARNING,
        Severity::InfoC => DiagnosticSeverity::INFORMATION,
        Severity::StyleC => DiagnosticSeverity::HINT,
    }
}

/// Run the full pipeline on `text`. `filename` picks the shell from its
/// extension and the rc files from its directory; with `ignore_rc` no rc or
/// editorconfig is read.
pub fn run_check(text: &str, filename: &str, ignore_rc: bool, s: &Settings) -> Vec<Finding> {
    let spec = CheckSpec {
        filename: filename.to_string(),
        script: text.to_string(),
        ignore_rc,
        shell_type_override: s.dialect,
        excluded_warnings: s.exclude.clone(),
        min_severity: s.min_severity,
        optional_checks: s.enable.clone(),
        ..empty_check_spec()
    };
    let result = check_script(&LspSystem, &spec);
    let lines = split_lines(text);
    result.comments.iter().map(|pc| to_finding(pc, &lines)).collect()
}

fn to_finding(pc: &PositionedComment, lines: &[&str]) -> Finding {
    let mut range = Range {
        start: to_lsp(lines, pc.start_pos.line, pc.start_pos.column),
        end: to_lsp(lines, pc.end_pos.line, pc.end_pos.column),
    };
    if range.end <= range.start {
        // Zero-width comment: underline one character so it stays visible.
        range.end = Position { line: range.start.line, character: range.start.character + 1 };
    }
    let fix = pc
        .fix
        .as_ref()
        .map(|f| {
            f.replacements
                .iter()
                .map(|r| TextEdit {
                    range: Range {
                        start: to_lsp(lines, r.start_pos.line, r.start_pos.column),
                        end: to_lsp(lines, r.end_pos.line, r.end_pos.column),
                    },
                    new_text: r.string.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    Finding {
        range,
        severity: lsp_severity(pc.comment.severity),
        code: pc.comment.code,
        message: pc.comment.message.clone(),
        fix,
    }
}

/// Edits that add `SCxxxx` to the disable directive for the line `line`
/// (0-based). If the line above is already a `# shellcheck disable=` line the
/// code is appended to it; otherwise a new directive line is inserted with the
/// same indentation. A shebang on line 1 is never displaced.
pub fn disable_edits(lines: &[&str], line: u32, code: i64) -> Vec<TextEdit> {
    use crate::position::line_utf16_len;
    let line = (line as usize).min(lines.len().saturating_sub(1));
    let tag = format!("SC{}", code);
    if line > 0 {
        let prev = lines[line - 1];
        let trimmed = prev.trim_start();
        if let Some(rest) = trimmed.strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("shellcheck").map(str::trim_start)) {
            if let Some(codes) = rest.strip_prefix("disable=") {
                let codes_now: Vec<&str> = codes.split(',').map(str::trim).collect();
                if codes_now.iter().any(|c| c.eq_ignore_ascii_case(&tag)) {
                    return vec![];
                }
                let indent = &prev[..prev.len() - trimmed.len()];
                let new_line = format!("{}# shellcheck disable={},{}", indent, codes.trim_end(), tag);
                return vec![TextEdit {
                    range: Range {
                        start: Position { line: (line - 1) as u32, character: 0 },
                        end: Position { line: (line - 1) as u32, character: line_utf16_len(prev) },
                    },
                    new_text: new_line,
                }];
            }
        }
    }
    let target = lines.get(line).copied().unwrap_or("");
    let indent: String = target.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let (insert_at, indent) = if line == 0 && lines[0].starts_with("#!") {
        (1usize, String::new())
    } else {
        (line, indent)
    };
    let pos = Position { line: insert_at as u32, character: 0 };
    vec![TextEdit {
        range: Range { start: pos, end: pos },
        new_text: format!("{}# shellcheck disable={}\n", indent, tag),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_directive_with_indentation() {
        let lines = ["if true; then", "    echo $x", "fi"];
        let e = disable_edits(&lines, 1, 2086);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].range.start, Position { line: 1, character: 0 });
        assert_eq!(e[0].new_text, "    # shellcheck disable=SC2086\n");
    }

    #[test]
    fn appends_to_existing_directive() {
        let lines = ["# shellcheck disable=SC2034", "x=$y"];
        let e = disable_edits(&lines, 1, 2086);
        assert_eq!(e[0].new_text, "# shellcheck disable=SC2034,SC2086");
        assert_eq!(e[0].range.end.line, 0);
    }

    #[test]
    fn keeps_shebang_on_top() {
        let lines = ["#!/bin/sh", "echo $x"];
        let e = disable_edits(&lines, 0, 2086);
        assert_eq!(e[0].range.start.line, 1);
    }
}
