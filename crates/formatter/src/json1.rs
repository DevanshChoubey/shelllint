// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Formatter/JSON1.hs (e11f964)
//
// DIVERGENCE: see json.rs; the shared comment/fix encoders live there since
// the Haskell instances are textually identical.

use std::io::Write;

use shellcheck_core::interface::{CheckResult, PositionedComment};

use crate::json::comments_json;
use crate::*;

pub struct Json1 {
    list: Vec<PositionedComment>,
}

pub fn format() -> Json1 {
    Json1 { list: vec![] }
}

impl Formatter for Json1 {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    fn on_result(&mut self, cr: &CheckResult, source: &str, _out: &mut dyn Write) {
        for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let mut new = make_non_virtual(group, source);
            new.append(&mut self.list);
            self.list = new;
        }
    }

    fn footer(&mut self, out: &mut dyn Write) {
        let mut s = String::from("{\"comments\":");
        comments_json(&self.list, &mut s);
        s.push('}');
        let _ = writeln!(out, "{}", s);
    }
}
