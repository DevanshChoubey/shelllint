// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/EditorConfig.hs (e11f964), plus the
// EditorConfig lookup of shellcheck.hs (`getEditorConfig`, `mergeConfigs`,
// `isEditorConfigRejection`, lines 529-580) behind a filesystem trait.
//
// Minimal support for reading shellcheck directives from EditorConfig
// style files (https://editorconfig.org/). Only the `shellcheck.*` keys
// of sections whose glob matches the file being checked are extracted,
// and turned into the same "key=value" directive syntax that is used in
// .shellcheckrc files.
//
// The .shellcheckrc parsing itself (`readConfigFile`, `readConfigKVs`) is in
// Parser.hs and lives in the parser crate; the CLI's real-filesystem
// `SystemInterface` implements `EditorConfigFs` and calls
// `get_editor_config` / `merge_configs` from its `get_config`.
//
// DIVERGENCE: EditorConfig.hs repeats `splitSections`, `parseHeader`,
// `allDirectives`, ... in the `where` clauses of several functions; here
// they are shared private helpers defined before their first user.
//
// Not ported: the TemplateHaskell `runTests` footer (the `prop_` definitions
// live in `crates/checker/tests/checker.rs`).

use crate::filepath::{combine, take_directory, take_file_name};
use shelllint_core::data::shell_for_executable;
use shelllint_core::regex::{matches, mk_regex};
use shelllint_parser::engine::is_space;

/// Haskell `lines`: split on '\n', without a trailing empty line. Unlike
/// `str::lines`, a '\r' is kept.
fn haskell_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.split('\n').collect();
    if v.last() == Some(&"") {
        v.pop();
    }
    v
}

/// A `shellcheck.<key> = <value>` directive: (1-based line, key, value).
type Directive = (usize, String, String);

/// `splitSections 1 (lines contents)`: each section's glob and its numbered
/// body lines. Lines before the first section header are skipped.
fn split_sections(contents: &str) -> Vec<(String, Vec<(usize, &str)>)> {
    let ls = haskell_lines(contents);
    let mut out = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        match parse_header(ls[i]) {
            Some(pat) => {
                let start = i + 1;
                let mut end = start;
                while end < ls.len() && parse_header(ls[end]).is_none() {
                    end += 1;
                }
                let body = (start..end).map(|j| (j + 1, ls[j])).collect();
                out.push((pat, body));
                i = end;
            }
            None => i += 1,
        }
    }
    out
}

fn parse_header(l: &str) -> Option<String> {
    let t = trim(drop_line_comment(l));
    let cs = t.strip_prefix('[')?;
    if !cs.is_empty() && cs.ends_with(']') { Some(cs[..cs.len() - 1].to_string()) } else { None }
}

/// All 'shellcheck.*' directives (as (line, key, value) tuples) found
/// in sections whose glob matches the file being checked.
fn all_directives(name: &str, sections: &[(String, Vec<(usize, &str)>)]) -> Vec<Directive> {
    sections
        .iter()
        .filter(|(pat, _)| matches_glob(pat, name))
        .flat_map(|(_, body)| body.iter().filter_map(|&(line, l)| to_directive_pair(line, l)))
        .collect()
}

fn to_directive_pair(line: usize, l: &str) -> Option<Directive> {
    let t = trim(drop_line_comment(l));
    let (key, value) = t.split_once('=')?;
    let key = trim(key);
    let value = trim(value);
    key.strip_prefix("shellcheck.").map(|k| (line, k.to_string(), value.to_string()))
}

fn has_comment_marker(value: &str) -> bool {
    value.chars().any(|c| c == '#' || c == ';')
}

/// Given the contents of an EditorConfig style file and the name of the
/// file being checked, return the shellcheck directives (as a
/// "key=value\n" delimited blob, suitable for feeding into the same
/// parser as .shellcheckrc) found in matching sections.
///
/// As per the EditorConfig spec, files are read top to bottom and
/// properties from later sections override those from earlier ones
/// (for the same key), so on conflicts the last matching section wins.
pub fn parse_editor_config(contents: &str, name: &str) -> String {
    // Only emit directives that the .shellcheckrc parser can handle
    // unambiguously (see EditorConfig.hs for the reasoning).
    let is_usable_directive = |key: &str, value: &str| {
        if key == "shell" {
            value.is_empty() || shell_for_executable(value).is_some()
        } else {
            !value.is_empty() && !has_comment_marker(value)
        }
    };
    let usable: Vec<Directive> = all_directives(name, &split_sections(contents))
        .into_iter()
        .filter(|(_, k, v)| is_usable_directive(k, v))
        .collect();

    // Keep only the last occurrence of each key, preserving the
    // relative order of the remaining (first-seen) entries.
    let mut last_wins: Vec<Directive> = Vec::new();
    for d in usable.into_iter().rev() {
        if !last_wins.iter().any(|e| e.1 == d.1) {
            last_wins.push(d);
        }
    }
    last_wins.reverse();

    // Render each directive at its original line in the config file, so
    // that any parse error (SC1134) is reported at the correct line.
    let mut prev_line = 0;
    let mut out = String::new();
    for (line, key, value) in last_wins {
        out.push_str(&"\n".repeat(line.saturating_sub(prev_line + 1)));
        out.push_str(&format!("{key}={value}\n"));
        prev_line = line;
    }
    out
}

/// Returns the 1-based line numbers of invalid 'shellcheck.*' directives:
/// 'shellcheck.shell=<x>' with a non-empty unknown shell, and any value
/// containing '#' or ';' (EditorConfig has no inline comments). Only
/// directives in sections whose glob matches the file are reported.
pub fn invalid_directive_lines(contents: &str, name: &str) -> Vec<usize> {
    let is_invalid_directive = |key: &str, value: &str| {
        if key == "shell" {
            !value.is_empty() && shell_for_executable(value).is_none()
        } else {
            has_comment_marker(value)
        }
    };
    all_directives(name, &split_sections(contents))
        .into_iter()
        .filter(|(_, k, v)| is_invalid_directive(k, v))
        .map(|(line, _, _)| line)
        .collect()
}

/// Build the directive blob contributed by a single EditorConfig file for
/// the given file being checked. Returns `None` if the file contributes
/// nothing (no matching section, or only empty values). Returns a blob of
/// "key=value\n" directives when the matching sections are valid, or a
/// single rejected line at the position of any invalid 'shellcheck.*'
/// directive so that the .shellcheckrc parser reports it as SC1134.
pub fn editor_config_directives(contents: &str, name: &str) -> Option<String> {
    let rejected_line = |n: usize| "\n".repeat(n.saturating_sub(1)) + "invalid editorconfig value";
    let mut bad_lines = invalid_directive_lines(contents, name);
    if !bad_lines.is_empty() {
        bad_lines.sort();
        bad_lines.dedup();
        Some(bad_lines.into_iter().map(|n| rejected_line(n) + "\n").collect())
    } else {
        let result = parse_editor_config(contents, name);
        if result.is_empty() { None } else { Some(result) }
    }
}

fn is_section_header(l: &str) -> bool {
    let t = trim(drop_line_comment(l));
    match t.strip_prefix('[') {
        Some(cs) => !cs.is_empty() && cs.ends_with(']'),
        None => false,
    }
}

fn pre_section_lines(contents: &str) -> Vec<&str> {
    haskell_lines(contents).into_iter().take_while(|l| !is_section_header(l)).collect()
}

fn root_value(l: &str) -> Option<String> {
    let t = trim(drop_line_comment(l));
    let (key, value) = t.split_once('=')?;
    if trim(key).to_lowercase() == "root" { Some(trim(value).to_lowercase()) } else { None }
}

/// Does the top-level (pre-section) part of an EditorConfig file
/// declare "root = true"? Per the spec, this stops the search for
/// further EditorConfig files in parent directories.
// DIVERGENCE: `map toLower` is per-character; `to_lowercase` may expand a
// few non-ASCII characters to several. Only the results "true"/"false"
// matter, and those are unaffected.
pub fn is_editor_config_root(contents: &str) -> bool {
    pre_section_lines(contents).into_iter().any(|l| root_value(l).as_deref() == Some("true"))
}

/// Returns the 1-based line numbers of invalid 'root' declarations,
/// i.e. root values other than true/false (such as 'root =').
pub fn invalid_root_lines(contents: &str) -> Vec<usize> {
    pre_section_lines(contents)
        .into_iter()
        .enumerate()
        .filter(|(_, l)| root_value(l).is_some_and(|v| v != "true" && v != "false"))
        .map(|(i, _)| i + 1)
        .collect()
}

/// EditorConfig does not allow inline comments; '#' and ';' starting
/// on the first non-whitespace character denote a full-line comment.
/// Lines not starting with '#' or ';' must be returned verbatim.
fn drop_line_comment(l: &str) -> &str {
    match l.trim_start_matches(is_space).chars().next() {
        Some('#') | Some(';') => "",
        _ => l,
    }
}

fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Does the (relative path of the) file match the given EditorConfig glob?
pub fn matches_glob(pattern: &str, name: &str) -> bool {
    matches(name, &mk_regex(&glob_to_regex_string(pattern)))
}

/// Translate an EditorConfig glob pattern into an anchored regex string.
/// Per the spec, patterns without a path separator are matched against
/// the file at any depth (as if prefixed with "**/").
pub fn glob_to_regex_string(pattern: &str) -> String {
    let prefix = if pattern.contains('/') { "" } else { "(.*/)?" };
    let chars: Vec<char> = pattern.chars().collect();
    format!("^{prefix}{}$", go(&chars))
}

fn go(s: &[char]) -> String {
    const REGEX_SPECIALS: &str = ".\\+()^$|";
    match s {
        [] => String::new(),
        ['*', '*', rest @ ..] => ".*".to_string() + &go(rest),
        ['*', rest @ ..] => "[^/]*".to_string() + &go(rest),
        ['?', rest @ ..] => "[^/]".to_string() + &go(rest),
        ['[', rest @ ..] => match rest.iter().position(|&c| c == ']') {
            Some(i) => format!("[{}]", translate_class(&rest[..i])) + &go(&rest[i + 1..]),
            None => "\\[".to_string() + &go(rest),
        },
        ['{', rest @ ..] => match find_matching_brace(rest) {
            Some((body, rest2)) => {
                let alts: Vec<String> = brace_alternatives(&body).iter().map(|a| go(a)).collect();
                build_alternation(&alts) + &go(rest2)
            }
            None => "\\{".to_string() + &go(rest),
        },
        [c, rest @ ..] => {
            let head = if REGEX_SPECIALS.contains(*c) { format!("\\{c}") } else { c.to_string() };
            head + &go(rest)
        }
    }
}

// DIVERGENCE: the Haskell only escapes '\\' and hands the class to
// regex-tdfa. The `regex` crate also treats '[' (nested class), "&&", "~~"
// and "--" (set operations) specially, so '[', '&', '~' and any '-' that
// cannot be a range ('-' first, last, or next to another '-') are escaped.
// An empty class ("[]" / "[!]"), which tdfa rejects (an `error` call in
// ShellCheck), becomes a class matching nothing / any character instead of
// a panic.
fn translate_class(cls: &[char]) -> String {
    let escape_class = |cs: &[char]| {
        let mut out = String::new();
        for (i, &c) in cs.iter().enumerate() {
            let literal_dash = c == '-' && (i == 0 || i + 1 == cs.len() || cs[i - 1] == '-' || cs[i + 1] == '-');
            if matches!(c, '\\' | '[' | '&' | '~') || literal_dash {
                out.push('\\');
            }
            out.push(c);
        }
        out
    };
    match cls {
        [] => "^\\x00-\\x{10FFFF}".to_string(),
        ['!'] => "\\x00-\\x{10FFFF}".to_string(),
        ['!', cs @ ..] => "^".to_string() + &escape_class(cs),
        cs => escape_class(cs),
    }
}

/// Scan past a balanced '{' ... '}' pair, returning the contents of
/// the braces (with any nested braces verbatim) and the remainder.
/// Returns `None` if the braces are unbalanced.
fn find_matching_brace(s: &[char]) -> Option<(Vec<char>, &[char])> {
    let mut d = 1;
    for (i, &c) in s.iter().enumerate() {
        if c == '}' {
            if d == 1 {
                return Some((s[..i].to_vec(), &s[i + 1..]));
            }
            d -= 1;
        } else if c == '{' {
            d += 1;
        }
    }
    None
}

/// Wrap a list of regex alternatives in an anchored group. Empty
/// alternatives are handled so the result is always a valid regex.
fn build_alternation(alts: &[String]) -> String {
    if alts.iter().all(|a| a.is_empty()) {
        String::new()
    } else if alts.iter().any(|a| a.is_empty()) {
        let non_empty: Vec<&str> = alts.iter().filter(|a| !a.is_empty()).map(String::as_str).collect();
        format!("({})?", non_empty.join("|"))
    } else {
        format!("({})", alts.join("|"))
    }
}

/// Expand EditorConfig brace alternatives: numeric ranges like '{1..3}',
/// or comma separated alternatives (commas inside nested braces are not
/// separators).
fn brace_alternatives(body: &[char]) -> Vec<Vec<char>> {
    if let Some(i) = body.iter().position(|&c| c == '.') {
        let (from_str, rest) = (&body[..i], &body[i + 1..]);
        if let ['.', to_str @ ..] = rest {
            if let (Some(from), Some(to)) = (read_int(from_str), read_int(to_str)) {
                return if from < to { (from..=to).map(|n| n.to_string().chars().collect()).collect() } else { vec![] };
            }
        }
    }
    split_top_level_commas(body)
}

fn split_top_level_commas(s: &[char]) -> Vec<Vec<char>> {
    let mut out = Vec::new();
    let mut acc = Vec::new();
    let mut d: usize = 0;
    for &c in s {
        match c {
            '{' => {
                d += 1;
                acc.push(c);
            }
            '}' => {
                d = d.saturating_sub(1);
                acc.push(c);
            }
            ',' if d == 0 => out.push(std::mem::take(&mut acc)),
            _ => acc.push(c),
        }
    }
    out.push(acc);
    out
}

/// `reads s :: [(Int, String)]` with nothing left over.
// DIVERGENCE: Haskell's `reads` also accepts parenthesised numbers and
// whitespace between '-' and the digits; only `[ws]['-']digits` is accepted
// here. A number that overflows `i64` gives `None` (no range expansion),
// where `reads :: Int` silently wraps.
fn read_int(s: &[char]) -> Option<i64> {
    let s: String = s.iter().collect();
    let t = s.trim_start_matches(is_space);
    let digits = t.strip_prefix('-').unwrap_or(t);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

// ---- shellcheck.hs: EditorConfig lookup (ioInterface's getConfig) ----------

/// The IO that shellcheck.hs's `getEditorConfig` does, so the lookup can be
/// run against a real or a mocked filesystem.
pub trait EditorConfigFs {
    /// `normalize`: `canonicalizePath`, or the path itself on failure.
    fn normalize(&self, path: &str) -> String;
    /// `readConfig file`: `Some((file, contents))` if the file exists
    /// (contents "" if it could not be read), else `None`.
    fn read_config(&self, file: &str) -> Option<(String, String)>;
    /// `getXdgDirectory XdgConfig "editorconfig.ini"`, `None` on failure.
    fn global_editor_config_path(&self) -> Option<String>;
}

/// `mergeConfigs filename rcResult ecResult`: the .shellcheckrc and the
/// EditorConfig blobs (name, contents) combined into what `siGetConfig`
/// returns.
pub fn merge_configs(
    rc_result: Option<(String, String)>,
    ec_result: Option<(String, String)>,
) -> Option<(String, String)> {
    match (rc_result, ec_result) {
        (None, None) => None,
        (Some((rc_path, rc)), None) => Some((rc_path, rc)),
        (None, Some((ec_path, ec))) => Some((ec_path, ec)),
        (Some((rc_path, rc)), Some((ec_path, ec))) => {
            if is_editor_config_rejection(&ec) {
                Some((ec_path, ec))
            } else {
                Some((rc_path, rc + "\n" + &ec))
            }
        }
    }
}

/// `isEditorConfigRejection ec`
pub fn is_editor_config_rejection(ec: &str) -> bool {
    haskell_lines(ec).into_iter().all(|l| trim(l).is_empty() || l == "invalid editorconfig value")
}

/// `getEditorConfig filename`: look for .editorconfig files in the target
/// file's directory and all its parents (as per the EditorConfig spec), plus
/// the global `${XDG_CONFIG_HOME}/editorconfig.ini` default. shellcheck.*
/// keys in matching sections are turned into directives. Returns the first
/// contributing file's name and all contributions concatenated.
pub fn get_editor_config(fs: &dyn EditorConfigFs, filename: &str) -> Option<(String, String)> {
    // Resolve the directory (to find .editorconfig files) but keep
    // the leaf filename as-is so that globs match the symlink name
    // rather than the resolved target.
    let name = take_file_name(filename);
    let dir = fs.normalize(&take_directory(filename));
    let path = combine(&dir, name);
    let mut all_configs = collect_dir_configs(fs, &dir);
    if let Some(global) = fs.global_editor_config_path().filter(|p| !p.is_empty()) {
        all_configs.extend(fs.read_config(&global));
    }
    let contributions: Vec<(String, String)> =
        all_configs.iter().flat_map(|(file, contents)| directives_for(&path, file, contents)).collect();
    let (source_file, _) = contributions.first()?;
    Some((source_file.clone(), contributions.iter().map(|(_, b)| b.as_str()).collect()))
}

/// For each EditorConfig file: report any invalid 'root' declaration (which
/// takes priority) as a rejected config blob, otherwise yield the matching
/// shellcheck.* directives.
fn directives_for(path: &str, file: &str, contents: &str) -> Option<(String, String)> {
    let relative = make_relative_to(&take_directory(file), path);
    match invalid_root_lines(contents).first() {
        Some(&bad_line) => {
            Some((file.to_string(), "\n".repeat(bad_line.saturating_sub(1)) + "invalid editorconfig value\n"))
        }
        None => editor_config_directives(contents, &relative).map(|blob| (file.to_string(), blob)),
    }
}

fn make_relative_to(dir: &str, path: &str) -> String {
    let with_slash = add_trailing_slash(dir);
    match path.strip_prefix(with_slash.as_str()) {
        Some(rest) => rest.to_string(),
        None => take_file_name(path).to_string(),
    }
}

// As upstream, this appends `/` on every platform: on Windows the prefix then
// does not match, and the glob is tested against the bare file name.
fn add_trailing_slash(dir: &str) -> String {
    if dir.is_empty() || dir.ends_with('/') { dir.to_string() } else { format!("{dir}/") }
}

fn collect_dir_configs(fs: &dyn EditorConfigFs, dir: &str) -> Vec<(String, String)> {
    let current = fs.read_config(&combine(dir, ".editorconfig"));
    let is_root = current.as_ref().is_some_and(|(_, c)| is_editor_config_root(c));
    let next = take_directory(dir);
    let rest = if next != dir && !is_root { collect_dir_configs(fs, &next) } else { vec![] };
    current.into_iter().chain(rest).collect()
}
