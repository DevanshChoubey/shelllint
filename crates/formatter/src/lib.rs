// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/Format.hs (e11f964)
//
// DIVERGENCE: the Haskell `Formatter` record of IO actions becomes a trait.
// `onResult` receives the `SystemInterface` as in Haskell, and writes to
// `out` instead of stdout. Haskell writes
// `onFailure` messages to stderr for tty/gcc/json/json1; those formatters
// still write to the process stderr, while checkstyle writes to `out`.

use std::io::{IsTerminal, Write};

use shellcheck_core::interface::{
    ColorOption, Fix, FormatterOptions, PositionedComment, Severity, SystemInterface,
};

pub mod checkstyle;
pub mod diff;
pub mod fixer;
pub mod gcc;
pub mod json;
pub mod json1;
pub mod quiet;
pub mod tty;

pub trait Formatter {
    fn header(&mut self, out: &mut dyn Write);
    fn on_result(
        &mut self,
        r: &shellcheck_core::interface::CheckResult,
        sys: &dyn SystemInterface,
        out: &mut dyn Write,
    );
    fn on_failure(&mut self, file: &str, msg: &str, out: &mut dyn Write);
    fn footer(&mut self, out: &mut dyn Write);

    // DIVERGENCE: Haskell's Quiet formatter calls `exitFailure` from inside
    // the callbacks. A library must not exit the process, so such a formatter
    // instead reports true here once it would have exited with status 1.
    fn exit_requested(&self) -> bool {
        false
    }
}

pub fn by_name(name: &str, opts: FormatterOptions) -> Option<Box<dyn Formatter>> {
    match name {
        "tty" => Some(Box::new(tty::format(opts))),
        "gcc" => Some(Box::new(gcc::format())),
        "json" => Some(Box::new(json::format())),
        "json1" => Some(Box::new(json1::format())),
        "checkstyle" => Some(Box::new(checkstyle::format())),
        "diff" => Some(Box::new(diff::format(opts))),
        "quiet" => Some(Box::new(quiet::format(opts))),
        _ => None,
    }
}

/// `siReadFile sys (Just True) name`, with a failed read giving "" (the
/// `either (const "") id result` the tty, gcc, json1 and checkstyle
/// formatters use).
pub fn read_or_empty(sys: &dyn SystemInterface, name: &str) -> String {
    sys.read_file(Some(true), name).unwrap_or_default()
}

pub fn source_file(pc: &PositionedComment) -> &str {
    &pc.start_pos.file
}
pub fn line_no(pc: &PositionedComment) -> i64 {
    pc.start_pos.line
}
pub fn end_line_no(pc: &PositionedComment) -> i64 {
    pc.end_pos.line
}
pub fn col_no(pc: &PositionedComment) -> i64 {
    pc.start_pos.column
}
pub fn end_col_no(pc: &PositionedComment) -> i64 {
    pc.end_pos.column
}
pub fn code_no(pc: &PositionedComment) -> i64 {
    pc.comment.code
}
pub fn message_text(pc: &PositionedComment) -> &str {
    &pc.comment.message
}

pub fn severity_text(pc: &PositionedComment) -> &'static str {
    match pc.comment.severity {
        Severity::ErrorC => "error",
        Severity::WarningC => "warning",
        Severity::InfoC => "info",
        Severity::StyleC => "style",
    }
}

/// Haskell `lines`: split on '\n' only; a trailing newline adds no empty line.
pub fn haskell_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.split('\n').collect();
    if v.last() == Some(&"") {
        v.pop();
    }
    v
}

/// NE.groupWith: group adjacent elements with equal keys (no sorting).
pub fn group_with<'a, K: PartialEq>(
    items: &'a [PositionedComment],
    key: impl Fn(&PositionedComment) -> K,
) -> Vec<&'a [PositionedComment]> {
    let mut groups = vec![];
    let mut start = 0;
    for i in 1..=items.len() {
        if i == items.len() || key(&items[i]) != key(&items[start]) {
            if start < items.len() {
                groups.push(&items[start..i]);
            }
            start = i;
        }
    }
    groups
}

/// Realign comments from a tabstop of 8 to 1
pub fn make_non_virtual(comments: &[PositionedComment], contents: &str) -> Vec<PositionedComment> {
    let list = haskell_lines(contents);
    comments
        .iter()
        .map(|c| {
            let mut fixed = fixer::remove_tab_stops_comment(c, &list);
            fixed.fix = c.fix.as_ref().map(|f| untabbed_fix(f, &list));
            fixed
        })
        .collect()
}

fn untabbed_fix(f: &Fix, arr: &[&str]) -> Fix {
    Fix {
        replacements: f
            .replacements
            .iter()
            .map(|r| fixer::remove_tab_stops_replacement(r, arr))
            .collect(),
    }
}

pub fn should_output_color(color_option: ColorOption) -> bool {
    match color_option {
        ColorOption::ColorAlways => true,
        ColorOption::ColorNever => false,
        ColorOption::ColorAuto => {
            let is_terminal = std::io::stdout().is_terminal();
            let term = std::env::var("TERM").ok();
            let windows = cfg!(windows);
            let dumb_term = matches!(term.as_deref(), None | Some("") | Some("dumb"));
            is_terminal && !windows && !dumb_term
        }
    }
}
