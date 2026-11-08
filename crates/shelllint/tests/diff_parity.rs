// SPDX-License-Identifier: GPL-3.0-or-later
//! Applying all fixes through the facade gives the same text as applying the
//! hunks of `shellcheck -f diff`.
#![cfg(feature = "serde")]

mod common;

use common::{fixture, require_cli, run_cli};
use shelllint::{Linter, Shell, apply_fixes};

/// Applies the hunks of a unified diff as printed by the diff formatter
/// (`--- a/..`, `+++ b/..`, `@@ -a,b +c,d @@`, ` `/`-`/`+` lines and
/// `\ No newline at end of file`) to `src`.
fn apply_diff(src: &str, diff: &str) -> String {
    // Old lines without their newline, and whether the last one had one.
    let old: Vec<&str> = src.split_inclusive('\n').map(|l| l.strip_suffix('\n').unwrap_or(l)).collect();
    let old_last_nl = src.is_empty() || src.ends_with('\n');
    let old_nl = |i: usize| !(i + 1 == old.len() && !old_last_nl);

    let mut out: Vec<(String, bool)> = vec![];
    let mut cursor = 0usize;
    let mut lines = diff.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        let Some(header) = line.strip_prefix("@@ -") else { continue };
        // "a,b +c,d @@": the old side starts at a and spans b lines.
        let old_spec = header.split(' ').next().unwrap();
        let (start, count) = old_spec.split_once(',').unwrap();
        let (start, count): (usize, usize) = (start.parse().unwrap(), count.parse().unwrap());
        let target = if count == 0 { start } else { start - 1 };
        while cursor < target {
            out.push((old[cursor].to_string(), old_nl(cursor)));
            cursor += 1;
        }
        while let Some(body) = lines.peek() {
            if body.starts_with("@@ -") || body.starts_with("--- ") {
                break;
            }
            let body = lines.next().unwrap();
            if body.starts_with("\\ No newline") {
                if let Some(last) = out.last_mut() {
                    last.1 = false;
                }
                continue;
            }
            let mut chars = body.chars();
            // The formatter ends its output with a blank line, which is not a hunk line.
            let Some(tag) = chars.next() else { continue };
            let text = chars.as_str();
            match tag {
                ' ' => {
                    out.push((text.to_string(), old_nl(cursor)));
                    cursor += 1;
                }
                '-' => cursor += 1,
                '+' => {
                    if let Some(last) = out.last_mut() {
                        last.1 = true;
                    }
                    out.push((text.to_string(), true));
                }
                other => panic!("unexpected diff line tag {other:?} in {body:?}"),
            }
        }
    }
    while cursor < old.len() {
        out.push((old[cursor].to_string(), old_nl(cursor)));
        cursor += 1;
    }
    out.iter().map(|(t, nl)| if *nl { format!("{t}\n") } else { t.clone() }).collect()
}

const CASES: &[(&str, &str, Shell)] = &[
    ("fix_basic.sh", "sh", Shell::Sh),
    ("no_trailing_newline.sh", "bash", Shell::Bash),
    ("tabs.sh", "bash", Shell::Bash),
    ("backticks.sh", "bash", Shell::Bash),
    ("function.sh", "sh", Shell::Sh),
];

#[test]
fn apply_all_fixes_matches_shellcheck_diff() {
    let Some(bin) = require_cli("apply_all_fixes_matches_shellcheck_diff") else { return };
    for &(name, cli_shell, shell) in CASES {
        let path = fixture(name);
        let src = std::fs::read_to_string(&path).unwrap();

        let diff = run_cli(&bin, &["--norc", "-o", "all", "-s", cli_shell, "-f", "diff"], &path);
        let expected = apply_diff(&src, &diff);

        let linter = Linter::builder().shell(shell).enable_all_optional().build();
        let diagnostics = linter.check_str(&src);
        let got = apply_fixes(&src, &diagnostics);

        assert_eq!(got, expected, "fixed output differs for {name}\n--- diff ---\n{diff}");
    }
}
