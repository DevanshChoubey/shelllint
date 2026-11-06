// SPDX-License-Identifier: GPL-3.0-only
// Benchmark driver (no Haskell counterpart): a minimal stand-in for the
// `shellcheck -f json1 --norc -o all FILE...` command line until the real CLI
// is wired. Reads each file, runs the full pipeline single-threaded per file
// and prints a json1-shaped report (file/line/column/level/code/message).
//
//   sc_bench FILE...            check each file in turn, print json1
//   sc_bench --quiet FILE...    same, but print only a one-line summary
//   sc_bench --parallel FILE... check files on one thread each (separate mode)
use shellcheck_checker::{check_script, decode_string};
use shellcheck_core::interface::{CheckSpec, Severity, empty_check_spec, mocked_system_interface};

fn level(s: Severity) -> &'static str {
    match s {
        Severity::ErrorC => "error",
        Severity::WarningC => "warning",
        Severity::InfoC => "info",
        Severity::StyleC => "style",
    }
}

fn esc(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

fn run(path: &str) -> (String, usize) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let spec = CheckSpec {
        filename: path.to_string(),
        script: decode_string(&bytes),
        ignore_rc: true,
        optional_checks: vec!["all".to_string()],
        ..empty_check_spec()
    };
    let res = check_script(&mocked_system_interface(vec![]), &spec);
    let mut out = String::new();
    for (i, pc) in res.comments.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"file\":\"{}\",\"line\":{},\"endLine\":{},\"column\":{},\"endColumn\":{},\"level\":\"{}\",\"code\":{},\"message\":\"",
            path, pc.start_pos.line, pc.end_pos.line, pc.start_pos.column, pc.end_pos.column,
            level(pc.comment.severity), pc.comment.code, 
        ));
        esc(&pc.comment.message, &mut out);
        out.push_str("\"}");
    }
    (out, res.comments.len())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut quiet = false;
    let mut parallel = false;
    args.retain(|a| match a.as_str() {
        "--quiet" => { quiet = true; false }
        "--parallel" => { parallel = true; false }
        _ => true,
    });
    let results: Vec<(String, usize)> = if parallel {
        let hs: Vec<_> = args.iter().cloned().map(|p| std::thread::spawn(move || run(&p))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    } else {
        args.iter().map(|p| run(p)).collect()
    };
    if quiet {
        println!("files={} comments={}", results.len(), results.iter().map(|r| r.1).sum::<usize>());
    } else {
        let parts: Vec<&str> = results.iter().map(|r| r.0.as_str()).filter(|s| !s.is_empty()).collect();
        println!("{{\"comments\":[{}]}}", parts.join(","));
    }
}
