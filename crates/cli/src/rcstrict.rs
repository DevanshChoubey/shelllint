// SPDX-License-Identifier: GPL-3.0-or-later
// Extension beyond ShellCheck: `color=` and `wiki-link-count=` in
// .shellcheckrc, and `--strict-rc`.
//
// DIVERGENCE: upstream ignores both rc keys (unknown rc keys are dropped
// silently) and has no `--strict-rc`.
//
// The two formatter keys are handled here, in the CLI, because they configure
// the formatter and not the parser: `strip_extras` blanks them out (keeping
// every line and column) before the text reaches the parser, and
// `rc_overrides` reads their values. A key with an invalid value is left in
// place, where the parser ignores it, exactly as today.

use std::collections::HashSet;
use std::sync::Mutex;

use shelllint_core::data::shell_for_executable;
use shelllint_core::interface::ColorOption;

use crate::Status;
use crate::options::Options;
use crate::sys;

/// Values `.shellcheckrc` can give for the formatter.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct RcExtras {
    pub color: Option<ColorOption>,
    pub wiki_link_count: Option<i64>,
}

fn extra_value(token: &str, extras: &mut RcExtras) -> bool {
    if let Some(v) = token.strip_prefix("color=") {
        let c = match v {
            "auto" => ColorOption::ColorAuto,
            "always" => ColorOption::ColorAlways,
            "never" => ColorOption::ColorNever,
            _ => return false,
        };
        extras.color = Some(c);
        true
    } else if let Some(v) = token.strip_prefix("wiki-link-count=") {
        if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        extras.wiki_link_count = Some(v.parse::<i64>().unwrap_or(i64::MAX));
        true
    } else {
        false
    }
}

/// The directives of an rc file as (line index, start, end, text), columns in
/// chars. Comments are skipped; a quoted value may contain whitespace.
fn tokens(text: &str) -> Vec<(usize, usize, usize, String)> {
    let mut out = vec![];
    for (n, line) in text.split('\n').enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            if chars[i] == '#' {
                break;
            }
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                if chars[i] == '=' && i + 1 < chars.len() && (chars[i + 1] == '"' || chars[i + 1] == '\'') {
                    let q = chars[i + 1];
                    i += 2;
                    while i < chars.len() && chars[i] != q {
                        i += 1;
                    }
                }
                i += 1;
            }
            let end = i.min(chars.len());
            out.push((n, start, end, chars[start..end].iter().collect()));
        }
    }
    out
}

/// Blank out the `color=` / `wiki-link-count=` directives of an rc file,
/// keeping every line and column. The text is returned unchanged when there
/// are none.
pub fn strip_extras(text: &str) -> (String, RcExtras) {
    let mut extras = RcExtras::default();
    if !text.contains("color=") && !text.contains("wiki-link-count=") {
        return (text.to_string(), extras);
    }
    let mut lines: Vec<Vec<char>> = text.split('\n').map(|l| l.chars().collect()).collect();
    for (n, start, end, token) in tokens(text) {
        if extra_value(&token, &mut extras) {
            for c in &mut lines[n][start..end] {
                *c = ' ';
            }
        }
    }
    (lines.into_iter().map(|l| l.into_iter().collect::<String>()).collect::<Vec<_>>().join("\n"), extras)
}

/// Keys the parser understands in an rc file. Add new keys here too.
const KNOWN_KEYS: &[&str] =
    &["disable", "enable", "source", "source-path", "shell", "extended-analysis", "external-sources", "severity", "external-vars"];

/// Problems `--strict-rc` rejects: (line, column, description).
fn find_problems(text: &str) -> Vec<(usize, usize, String)> {
    let mut found = vec![];
    for (n, start, _, token) in tokens(text) {
        let (key, value) = token.split_once('=').unwrap_or((token.as_str(), ""));
        let msg = if key == "color" || key == "wiki-link-count" {
            if extra_value(&token, &mut RcExtras::default()) {
                continue;
            }
            format!("invalid value for '{}'", key)
        } else if !KNOWN_KEYS.contains(&key) || !token.contains('=') {
            format!("unknown directive '{}'", key)
        } else if key == "shell" {
            let v = value.trim_matches(|c| c == '"' || c == '\'');
            if shell_for_executable(v).is_some() {
                continue;
            }
            format!("invalid shell value '{}'", v)
        } else {
            continue;
        };
        found.push((n + 1, start + 1, msg));
    }
    found
}

/// The rc file (path, text) that applies to `filename`, without any warning output.
fn rc_text_for(options: &Options, filename: &str) -> Option<(String, String)> {
    sys::quiet_rc_text(options.rcfile.as_deref(), filename)
}

/// Apply rc `color=` / `wiki-link-count=` where the command line did not
/// give the option (a flag always wins). The rc file is the one that applies
/// to the first input file.
pub fn apply_rc_overrides(options: &mut Options, flag_color: bool, flag_wiki: bool, first_file: Option<&str>) {
    if options.check_spec.ignore_rc || (flag_color && flag_wiki) {
        return;
    }
    let Some(file) = first_file else { return };
    let Some((_, text)) = rc_text_for(options, file) else { return };
    let (_, extras) = strip_extras(&text);
    if let (false, Some(c)) = (flag_color, extras.color) {
        options.formatter_options.color_option = c;
    }
    if let (false, Some(n)) = (flag_wiki, extras.wiki_link_count) {
        options.formatter_options.wiki_link_count = n;
    }
}

/// `--strict-rc`: a `--rcfile` that cannot be read is an error.
pub fn check_rcfile_readable(options: &Options) -> Result<(), Status> {
    if !options.strict_rc || options.check_spec.ignore_rc {
        return Ok(());
    }
    let Some(file) = &options.rcfile else { return Ok(()) };
    // Never read a fifo or process substitution here: the real read needs it.
    let problem = if !sys::file_exists(file) {
        Some("No such file".to_string())
    } else if std::fs::metadata(file).map(|m| m.is_file()).unwrap_or(false) {
        std::fs::File::open(file).err().map(|e| sys::show_io_error(file, &e))
    } else {
        None
    };
    match problem {
        None => Ok(()),
        Some(p) => {
            crate::print_err(&format!("shelllint: --strict-rc: unable to read --rcfile {}: {}", file, p));
            Err(Status::RuntimeException)
        }
    }
}

static REPORTED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// `--strict-rc`: an unknown key, or an invalid `shell=` (or `color=` /
/// `wiki-link-count=`) value in the rc file that applies to `filename`.
/// Prints each problem (once per rc file) and returns true if there was any.
pub fn rc_has_problems(options: &Options, filename: &str) -> bool {
    let Some((rc_path, text)) = rc_text_for(options, filename) else { return false };
    let problems = find_problems(&text);
    if problems.is_empty() {
        return false;
    }
    let mut seen = REPORTED.lock().unwrap();
    if seen.get_or_insert_with(HashSet::new).insert(rc_path.clone()) {
        for (line, col, msg) in problems {
            crate::print_err(&format!("shelllint: --strict-rc: {}:{}:{}: {}", rc_path, line, col, msg));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_valid_extras() {
        let (s, e) = strip_extras("disable=SC1 color=always\n# color=never\nwiki-link-count=2\ncolor=red\n");
        let expect = format!("disable=SC1 {}\n# color=never\n{}\ncolor=red\n", " ".repeat(12), " ".repeat(17));
        assert_eq!(s, expect);
        assert_eq!(e, RcExtras { color: Some(ColorOption::ColorAlways), wiki_link_count: Some(2) });
        assert_eq!(strip_extras("disable=SC1\n").0, "disable=SC1\n");
        let p = find_problems("disable=SC1\ncolor=red x=1 shell=nope shell=bash\n");
        assert_eq!(p.len(), 3, "{:?}", p);
        assert_eq!((p[0].0, p[0].1), (2, 1));
    }
}
