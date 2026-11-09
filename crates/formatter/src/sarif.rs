// SPDX-License-Identifier: GPL-3.0-or-later
// An extension beyond ShellCheck (upstream issue #2405): SARIF 2.1.0 output.
//
// One document for all files, written by hand like the other JSON formatters.
// Positions are the ones json1 reports (tab stops realigned to 1).

use std::io::Write;

use shelllint_core::interface::{CheckResult, PositionedComment, Severity, SystemInterface};

use crate::json::json_string;
use crate::tty::WIKI_LINK;
use crate::*;

const SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
const INFORMATION_URI: &str = "https://github.com/DevanshChoubey/shelllint";

pub struct Sarif {
    list: Vec<PositionedComment>,
}

pub fn format() -> Sarif {
    Sarif { list: vec![] }
}

/// SARIF `level` for a ShellCheck severity.
fn level(s: Severity) -> &'static str {
    match s {
        Severity::ErrorC => "error",
        Severity::WarningC => "warning",
        Severity::InfoC | Severity::StyleC => "note",
    }
}

/// A file name as a SARIF `uri`: forward slashes, everything outside the URI
/// unreserved set percent-encoded (as UTF-8), relative paths left relative and
/// absolute ones given as `file://` URIs.
pub fn path_to_uri(path: &str) -> String {
    let path: String = if cfg!(windows) { path.replace('\\', "/") } else { path.to_string() };
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let mut out = String::new();
    if drive && cfg!(windows) {
        out.push_str("file:///");
    } else if path.starts_with('/') {
        out.push_str("file://");
    }
    for (i, &b) in bytes.iter().enumerate() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/')
            || (b == b':' && i == 1 && drive && cfg!(windows));
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn rule_id(code: i64) -> String {
    format!("SC{}", code)
}

impl Formatter for Sarif {
    fn header(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, file: &str, msg: &str, _out: &mut dyn Write) {
        eprintln!("{}: {}", file, msg);
    }

    fn on_result(&mut self, cr: &CheckResult, sys: &dyn SystemInterface, _out: &mut dyn Write) {
        for group in group_with(&cr.comments, |c| source_file(c).to_string()) {
            let contents = read_or_empty(sys, source_file(&group[0]));
            self.list.extend(make_non_virtual(group, &contents));
        }
    }

    fn footer(&mut self, out: &mut dyn Write) {
        let mut codes: Vec<i64> = self.list.iter().map(code_no).collect();
        codes.sort_unstable();
        codes.dedup();

        let mut s = format!("{{\"$schema\":\"{}\",\"version\":\"2.1.0\",\"runs\":[{{", SCHEMA);
        s.push_str(&format!(
            "\"tool\":{{\"driver\":{{\"name\":\"shelllint\",\"version\":\"{}\",\"informationUri\":\"{}\",\"rules\":[",
            env!("CARGO_PKG_VERSION"),
            INFORMATION_URI
        ));
        for (i, code) in codes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let id = rule_id(*code);
            s.push_str(&format!("{{\"id\":\"{id}\",\"helpUri\":\"{WIKI_LINK}{id}\"}}"));
        }
        s.push_str("]}},\"results\":[");
        for (i, c) in self.list.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let code = code_no(c);
            let index = codes.binary_search(&code).unwrap_or(0);
            s.push_str(&format!(
                "{{\"ruleId\":\"{}\",\"ruleIndex\":{},\"level\":\"{}\",",
                rule_id(code),
                index,
                level(c.comment.severity)
            ));
            s.push_str("\"message\":{\"text\":");
            json_string(message_text(c), &mut s);
            s.push_str("},\"locations\":[{\"physicalLocation\":{\"artifactLocation\":{\"uri\":");
            json_string(&path_to_uri(source_file(c)), &mut s);
            s.push_str(&format!(
                "}},\"region\":{{\"startLine\":{},\"startColumn\":{},\"endLine\":{},\"endColumn\":{}}}}}}}]}}",
                line_no(c),
                col_no(c),
                end_line_no(c),
                end_col_no(c)
            ));
        }
        s.push_str("]}]}");
        let _ = writeln!(out, "{}", s);
    }
}

#[cfg(test)]
mod tests {
    use super::path_to_uri;

    #[test]
    fn uris_are_percent_encoded() {
        assert_eq!(path_to_uri("a/b.sh"), "a/b.sh");
        assert_eq!(path_to_uri("my dir/a b#c.sh"), "my%20dir/a%20b%23c.sh");
        assert_eq!(path_to_uri("/abs/x.sh"), "file:///abs/x.sh");
        assert_eq!(path_to_uri("-"), "-");
        assert_eq!(path_to_uri("\u{e9}.sh"), "%C3%A9.sh");
    }
}
