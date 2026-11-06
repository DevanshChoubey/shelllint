// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/TTY.hs (e11f964)

use std::io::Write;

use shellcheck_core::interface::{
    CheckResult, Code, ColorOption, Fix, FormatterOptions, PositionedComment, Severity, SystemInterface,
};

use crate::fixer::{apply_fix, map_positions, merge_fixes};
use crate::*;

pub const WIKI_LINK: &str = "https://www.shellcheck.net/wiki/";

// An arbitrary Ord thing to order warnings
type Ranking = (char, Severity, Code);

pub struct Tty {
    options: FormatterOptions,
    top_errors: Vec<(Ranking, Code, String)>,
}

pub fn format(options: FormatterOptions) -> Tty {
    Tty { options, top_errors: vec![] }
}

fn color_for_level(level: &str) -> i32 {
    match level {
        "error" => 31,   // red
        "warning" => 33, // yellow
        "info" => 32,    // green
        "style" => 32,   // green
        "verbose" => 32, // green
        "message" => 1,  // bold
        "source" => 0,   // none
        _ => 0,          // none
    }
}

fn rank_error(err: &PositionedComment) -> Ranking {
    // A list of the most generic, least directly helpful error codes to downrank.
    const UNINTERESTING: [Code; 10] = [1009, 1019, 1036, 1047, 1062, 1070, 1072, 1073, 1088, 1089];
    let ranking = if UNINTERESTING.contains(&err.comment.code) { 'Z' } else { 'A' };
    (ranking, err.comment.severity, err.comment.code)
}

fn append_comments(err_ref: &mut Vec<(Ranking, Code, String)>, comments: &[PositionedComment], max: i64) {
    let mut all: Vec<(Ranking, Code, String)> = err_ref.clone();
    all.extend(comments.iter().map(|x| (rank_error(x), x.comment.code, x.comment.message.clone())));
    all.sort();
    // nubBy on the ranking only; the list is sorted by ranking first.
    let mut nubbed: Vec<(Ranking, Code, String)> = vec![];
    for x in all {
        if !nubbed.iter().any(|y| y.0 == x.0) {
            nubbed.push(x);
        }
    }
    nubbed.truncate(max.max(0) as usize);
    *err_ref = nubbed;
}

type ColorFunc = Box<dyn Fn(&str, &str) -> String>;

fn get_color_func(color_option: ColorOption) -> ColorFunc {
    if should_output_color(color_option) {
        Box::new(|level, comment| format!("{}{}{}", ansi(color_for_level(level)), comment, ansi(0)))
    } else {
        Box::new(|_, comment| comment.to_string())
    }
}

fn ansi(n: i32) -> String {
    format!("\x1B[{}m", n)
}

impl Formatter for Tty {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn footer(&mut self, out: &mut dyn Write) {
        // outputWiki
        if !self.top_errors.is_empty() {
            let _ = writeln!(out, "For more information:");
            for (_, code, msg) in &self.top_errors {
                let _ = writeln!(out, "  {}SC{} -- {}", WIKI_LINK, code, shorten(msg));
            }
        }
    }

    fn on_failure(&mut self, file: &str, error: &str, _out: &mut dyn Write) {
        let color = get_color_func(self.options.color_option);
        eprintln!("{}", color("error", &format!("{}: {}", file, error)));
    }

    fn on_result(&mut self, result: &CheckResult, sys: &dyn SystemInterface, out: &mut dyn Write) {
        let color = get_color_func(self.options.color_option);
        let comments = &result.comments;
        append_comments(&mut self.top_errors, comments, self.options.wiki_link_count);
        for group in group_with(comments, |c| source_file(c).to_string()) {
            output_for_file(&color, sys, group, out);
        }
    }
}

fn shorten(msg: &str) -> String {
    const LIMIT: usize = 36;
    if msg.chars().count() < LIMIT {
        msg.to_string()
    } else {
        let mut s: String = msg.chars().take(LIMIT - 3).collect();
        s.push_str("...");
        s
    }
}

fn output_for_file(color: &ColorFunc, sys: &dyn SystemInterface, comments: &[PositionedComment], out: &mut dyn Write) {
    let file_name = source_file(&comments[0]);
    let contents = read_or_empty(sys, file_name);
    let file_lines = haskell_lines(&contents);
    let line_count = file_lines.len() as i64;
    for comments_for_line in group_with(comments, line_no) {
        let line_num = line_no(&comments_for_line[0]);
        let line = if line_num < 1 || line_num > line_count { "" } else { file_lines[(line_num - 1) as usize] };
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", color("message", &format!("In {} line {}:", file_name, line_num)));
        let _ = writeln!(out, "{}", color("source", line));
        for c in comments_for_line {
            let _ = writeln!(out, "{}", color(severity_text(c), &cute_indent(c)));
        }
        let _ = writeln!(out);
        show_fixed_string(color, comments_for_line, &file_lines, out);
    }
}

// Pick out only the lines necessary to show a fix in action
fn slice_file<'a>(fix: &Fix, lines: &[&'a str]) -> (Fix, Vec<&'a str>) {
    let (mut min_line, mut max_line) = (i64::MAX, i64::MIN);
    for r in &fix.replacements {
        for pos in [&r.start_pos, &r.end_pos] {
            min_line = min_line.min(pos.line);
            max_line = max_line.max(pos.line);
        }
    }
    let adjust = |pos: &shellcheck_core::interface::Position| {
        let mut p = pos.clone();
        p.line = pos.line - min_line + 1;
        p
    };
    let n = max_line.saturating_sub(min_line).saturating_add(1);
    let excerpt: Vec<&'a str> = (1..=n.max(0))
        .map(|x| {
            let i = x + min_line - 1;
            if i >= 1 { lines.get((i - 1) as usize).copied().unwrap_or("") } else { "" }
        })
        .collect();
    (map_positions(adjust, fix), excerpt)
}

fn show_fixed_string(color: &ColorFunc, comments: &[PositionedComment], file_lines: &[&str], out: &mut dyn Write) {
    let fixes: Vec<Fix> = comments.iter().filter_map(|c| c.fix.clone()).collect();
    if fixes.is_empty() {
        return;
    }
    // Folding automatically removes overlap
    let merged_fix = merge_fixes(&fixes);
    // We show the complete, associated fixes, whether or not it includes this
    // and/or other unrelated lines.
    let (excerpt_fix, excerpt) = slice_file(&merged_fix, file_lines);
    // in the spirit of error prone
    let _ = writeln!(out, "{}", color("message", "Did you mean:"));
    // putStrLn $ unlines ...
    let mut s = String::new();
    for l in apply_fix(&excerpt_fix, &excerpt) {
        s.push_str(&l);
        s.push('\n');
    }
    let _ = writeln!(out, "{}", s);
}

fn cute_indent(comment: &PositionedComment) -> String {
    let arrow = |n: i64| format!("^{}^", "-".repeat((n - 2).max(0) as usize));
    let same_line = line_no(comment) == end_line_no(comment);
    let delta = end_col_no(comment) - col_no(comment);
    let make_arrow = if same_line && delta > 2 && delta < 32 { arrow(delta) } else { "^--".to_string() };
    format!(
        "{}{} SC{} ({}): {}",
        " ".repeat((col_no(comment) - 1).max(0) as usize),
        make_arrow,
        code_no(comment),
        severity_text(comment),
        message_text(comment)
    )
}
