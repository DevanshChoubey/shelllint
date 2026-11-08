// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/JSON.hs (e11f964)
//
// DIVERGENCE: JSON is written by hand instead of through serde_json so that
// key order and escaping match Aeson byte for byte: comments use the
// `toEncoding` order, while `Fix`/`Replacement` go through `toJSON` and thus
// come out with alphabetically sorted keys.

use std::io::Write;

use shelllint_core::interface::{CheckResult, Fix, InsertionPoint, PositionedComment, Replacement, SystemInterface};

use crate::*;

pub struct Json {
    list: Vec<PositionedComment>,
}

pub fn format() -> Json {
    Json { list: vec![] }
}

/// Aeson string escaping: `"` `\` `\n` `\r` `\t` get short escapes, other
/// control characters below 0x20 become \u00xx, everything else is raw UTF-8.
pub(crate) fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn replacement_json(r: &Replacement, out: &mut String) {
    // Keys sorted, as Aeson's `object` yields.
    out.push_str(&format!("{{\"column\":{},", r.start_pos.column));
    out.push_str(&format!("\"endColumn\":{},", r.end_pos.column));
    out.push_str(&format!("\"endLine\":{},", r.end_pos.line));
    out.push_str(match r.insertion_point {
        InsertionPoint::InsertBefore => "\"insertionPoint\":\"beforeStart\",",
        InsertionPoint::InsertAfter => "\"insertionPoint\":\"afterEnd\",",
    });
    out.push_str(&format!("\"line\":{},", r.start_pos.line));
    out.push_str(&format!("\"precedence\":{},", r.precedence));
    out.push_str("\"replacement\":");
    json_string(&r.string, out);
    out.push('}');
}

fn fix_json(f: &Fix, out: &mut String) {
    out.push_str("{\"replacements\":[");
    for (i, r) in f.replacements.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        replacement_json(r, out);
    }
    out.push_str("]}");
}

pub(crate) fn comment_json(c: &PositionedComment, out: &mut String) {
    out.push_str("{\"file\":");
    json_string(source_file(c), out);
    out.push_str(&format!(
        ",\"line\":{},\"endLine\":{},\"column\":{},\"endColumn\":{},\"level\":",
        c.start_pos.line, c.end_pos.line, c.start_pos.column, c.end_pos.column
    ));
    json_string(severity_text(c), out);
    out.push_str(&format!(",\"code\":{},\"message\":", c.comment.code));
    json_string(&c.comment.message, out);
    out.push_str(",\"fix\":");
    match &c.fix {
        None => out.push_str("null"),
        Some(f) => fix_json(f, out),
    }
    out.push('}');
}

pub(crate) fn comments_json(list: &[PositionedComment], out: &mut String) {
    out.push('[');
    for (i, c) in list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        comment_json(c, out);
    }
    out.push(']');
}

impl Formatter for Json {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    // As in the Haskell, every group prepends *all* of the result's comments.
    fn on_result(&mut self, cr: &CheckResult, _sys: &dyn SystemInterface, _out: &mut dyn Write) {
        for _group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let mut new = cr.comments.clone();
            new.append(&mut self.list);
            self.list = new;
        }
    }

    fn footer(&mut self, out: &mut dyn Write) {
        let mut s = String::new();
        comments_json(&self.list, &mut s);
        let _ = writeln!(out, "{}", s);
    }
}
