// SPDX-License-Identifier: GPL-3.0-or-later
// File discovery (`-r`, `--exclude-path`) and changed-line filtering
// (`--changed-since`): extensions beyond upstream ShellCheck. None of this
// runs unless one of those options is given.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use shelllint_core::interface::CheckResult;

use crate::options::Options;

/// Directories `-r` never descends into.
const SKIPPED_DIRS: [&str; 4] = [".git", ".hg", ".svn", "node_modules"];

/// File name extensions that mark a shell script.
const SCRIPT_EXTENSIONS: [&str; 5] = ["sh", "bash", "dash", "ksh", "bats"];

/// Interpreters a shebang may name.
const SCRIPT_SHELLS: [&str; 5] = ["sh", "bash", "dash", "ksh", "busybox"];

/// How much of a file is read to look for a shebang.
const SHEBANG_BYTES: u64 = 256;

// ---------------------------------------------------------------------------
// Glob matching

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tok {
    Lit(char),
    /// `?`: one character, not `/`.
    One,
    /// `*`: any run of characters within one segment.
    Star,
    /// `**`: any run of characters, `/` included.
    Deep,
    /// `**/`: any number of whole directories, including none.
    DeepSlash,
}

fn tokenize(pattern: &str) -> Vec<Tok> {
    let cs: Vec<char> = pattern.chars().collect();
    let mut toks = vec![];
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            '?' => toks.push(Tok::One),
            '*' => {
                let mut j = i;
                while j < cs.len() && cs[j] == '*' {
                    j += 1;
                }
                if j - i >= 2 {
                    if cs.get(j) == Some(&'/') {
                        toks.push(Tok::DeepSlash);
                        j += 1;
                    } else {
                        toks.push(Tok::Deep);
                    }
                } else {
                    toks.push(Tok::Star);
                }
                i = j;
                continue;
            }
            c => toks.push(Tok::Lit(c)),
        }
        i += 1;
    }
    toks
}

fn match_toks(p: &[Tok], s: &[char]) -> bool {
    let Some((first, rest)) = p.split_first() else {
        return s.is_empty();
    };
    match first {
        Tok::Lit(c) => s.first() == Some(c) && match_toks(rest, &s[1..]),
        Tok::One => matches!(s.first(), Some(&c) if c != '/') && match_toks(rest, &s[1..]),
        Tok::Star => {
            let mut i = 0;
            loop {
                if match_toks(rest, &s[i..]) {
                    return true;
                }
                if i >= s.len() || s[i] == '/' {
                    return false;
                }
                i += 1;
            }
        }
        Tok::Deep => (0..=s.len()).any(|i| match_toks(rest, &s[i..])),
        Tok::DeepSlash => match_toks(rest, s) || (0..s.len()).any(|i| s[i] == '/' && match_toks(rest, &s[i + 1..])),
    }
}

/// Whole-string glob match: `*` stays inside one `/` segment, `**` crosses
/// segments (`**/` also matches no directory at all), `?` is one character.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    match_toks(&tokenize(pattern), &text)
}

/// A path as the globs see it: relative to the current directory, `/`
/// separated, with `.` and empty components dropped.
fn relative_components(path: &str) -> Vec<String> {
    let mut p = if cfg!(windows) { path.replace('\\', "/") } else { path.to_string() };
    if Path::new(&p).is_absolute() {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd = cwd.to_string_lossy().replace('\\', "/");
            let cwd = cwd.trim_end_matches('/');
            if let Some(rest) = p.strip_prefix(cwd) {
                if rest.starts_with('/') {
                    p = rest.to_string();
                }
            }
        }
    }
    p.split('/').filter(|c| !c.is_empty() && *c != ".").map(String::from).collect()
}

/// Is `path` (a file or a directory) matched by any `--exclude-path` glob? A
/// glob without `/` is tried against each path component; one with `/` is
/// tried against the whole path and every leading directory of it, so a glob
/// that names a directory excludes everything below it.
pub fn is_excluded(globs: &[&str], path: &str) -> bool {
    if globs.is_empty() {
        return false;
    }
    let comps = relative_components(path);
    globs.iter().any(|g| {
        let g = g.trim_start_matches("./").trim_start_matches('/').trim_end_matches('/');
        if g.is_empty() {
            return false;
        }
        if g.contains('/') {
            (1..=comps.len()).any(|k| glob_match(g, &comps[..k].join("/")))
        } else {
            comps.iter().any(|c| glob_match(g, c))
        }
    })
}

// ---------------------------------------------------------------------------
// Script detection and walking

/// Does a first line name a shell? Handles `#!/bin/sh`, `#!/usr/bin/env bash`
/// and `#!/usr/bin/env -S bash -e`.
pub fn shebang_is_shell(head: &[u8]) -> bool {
    let line = head.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let Some(rest) = line.strip_prefix(b"#!") else {
        return false;
    };
    let line = String::from_utf8_lossy(rest);
    let mut toks = line.split_whitespace();
    let Some(first) = toks.next() else {
        return false;
    };
    let base = |t: &str| t.rsplit('/').next().unwrap_or(t).to_string();
    let mut prog = base(first);
    if prog == "env" {
        let mut interp = None;
        while let Some(t) = toks.next() {
            if t == "-u" || t == "--unset" || t == "-C" || t == "--chdir" {
                toks.next();
            } else if t.starts_with('-') || t.contains('=') {
                continue;
            } else {
                interp = Some(t);
                break;
            }
        }
        match interp {
            Some(t) => prog = base(t),
            None => return false,
        }
    }
    SCRIPT_SHELLS.contains(&prog.as_str())
}

fn is_shell_script(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some((_, ext)) = name.rsplit_once('.') {
        if SCRIPT_EXTENSIONS.contains(&ext) {
            return true;
        }
    }
    use std::io::Read;
    let mut head = vec![];
    match std::fs::File::open(path) {
        Ok(f) => {
            let _ = f.take(SHEBANG_BYTES).read_to_end(&mut head);
        }
        Err(_) => return false,
    }
    shebang_is_shell(&head)
}

fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') { format!("{}{}", dir, name) } else { format!("{}/{}", dir, name) }
}

fn walk(dir: &str, globs: &[&str], out: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("shelllint: cannot read directory {}: {}", dir, e);
            return;
        }
    };
    let mut items: Vec<(OsString, std::fs::FileType)> =
        entries.flatten().filter_map(|e| e.file_type().ok().map(|t| (e.file_name(), t))).collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, ty) in items {
        // `file_type` does not follow links: symlinks are never walked.
        let name = name.to_string_lossy().into_owned();
        let path = join(dir, &name);
        if ty.is_dir() {
            if !SKIPPED_DIRS.contains(&name.as_str()) && !is_excluded(globs, &path) {
                walk(&path, globs, out);
            }
        } else if ty.is_file() && !is_excluded(globs, &path) && is_shell_script(&path) {
            out.push(path);
        }
    }
}

/// Expand directories (when `recursive`) and drop excluded paths. `-` is
/// passed through untouched.
pub fn expand_files(files: &[String], recursive: bool, globs: &[&str]) -> Vec<String> {
    let mut out = vec![];
    for f in files {
        if f == "-" {
            out.push(f.clone());
        } else if is_excluded(globs, f) {
            continue;
        } else if recursive && std::fs::symlink_metadata(f).map(|m| m.is_dir()).unwrap_or(false) {
            walk(f, globs, &mut out);
        } else {
            out.push(f.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// --changed-since

/// Changed line ranges (inclusive) per file name as given on the command line.
pub type ChangedLines = HashMap<String, Vec<(i64, i64)>>;

/// Parse the added/changed ranges out of `git diff --unified=0` output.
pub fn parse_hunks(diff: &str) -> Vec<(i64, i64)> {
    let mut ranges = vec![];
    for line in diff.lines() {
        let Some(rest) = line.strip_prefix("@@ -") else {
            continue;
        };
        let Some(plus) = rest.split(" +").nth(1) else {
            continue;
        };
        let spec = plus.split(' ').next().unwrap_or("");
        let (start, count) = match spec.split_once(',') {
            Some((a, b)) => (a.parse::<i64>(), b.parse::<i64>()),
            None => (spec.parse::<i64>(), Ok(1)),
        };
        if let (Ok(start), Ok(count)) = (start, count) {
            if count > 0 {
                ranges.push((start, start + count - 1));
            }
        }
    }
    ranges
}

fn git(dir: &str, args: &[&str]) -> Result<(bool, String, String), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git for --changed-since: {}", e))?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).trim().to_string(),
    ))
}

/// Work out the changed lines of each file relative to `rev`. Errors are
/// printable messages (not a repository, unknown revision, no git).
pub fn changed_lines(rev: &str, files: &[String]) -> Result<ChangedLines, String> {
    let mut map = ChangedLines::new();
    for file in files {
        let path = Path::new(file);
        if file == "-" || path.is_dir() || !path.exists() {
            continue;
        }
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_string_lossy().into_owned(),
            _ => ".".to_string(),
        };
        let base = path.file_name().map(|b| b.to_string_lossy().into_owned()).unwrap_or_default();
        let (ok, _, err) = git(&dir, &["rev-parse", "--is-inside-work-tree"])?;
        if !ok {
            return Err(format!("--changed-since: {} is not in a git repository ({})", file, err));
        }
        let commit = format!("{}^{{commit}}", rev);
        let (ok, _, _) = git(&dir, &["rev-parse", "--verify", "--quiet", &commit])?;
        if !ok {
            return Err(format!("--changed-since: unknown git revision '{}'", rev));
        }
        let (tracked, _, _) = git(&dir, &["ls-files", "--error-unmatch", "--", &base])?;
        if !tracked {
            map.insert(file.clone(), vec![(1, i64::MAX)]);
            continue;
        }
        let (ok, diff, err) = git(&dir, &["diff", "--unified=0", "--no-color", "--no-ext-diff", rev, "--", &base])?;
        if !ok {
            return Err(format!("--changed-since: git diff failed for {}: {}", file, err));
        }
        map.insert(file.clone(), parse_hunks(&diff));
    }
    Ok(map)
}

/// Drop the findings that do not touch a changed line. Findings located in
/// other files (sourced scripts) are dropped too, except the malformed
/// EditorConfig error, which must still fail the run.
pub fn keep_changed(options: &Options, mut result: CheckResult) -> CheckResult {
    let Some(ranges) = options.changed_lines.as_ref().and_then(|m| m.get(&result.filename)) else {
        return result;
    };
    let name = result.filename.clone();
    result.comments.retain(|pc| {
        if pc.start_pos.file != name {
            return pc.comment.code == 1134 && pc.start_pos.file.ends_with(".editorconfig");
        }
        let (lo, hi) = (pc.start_pos.line, pc.end_pos.line.max(pc.start_pos.line));
        ranges.iter().any(|&(a, b)| lo <= b && hi >= a)
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_basics() {
        assert!(glob_match("*.sh", "a.sh"));
        assert!(!glob_match("*.sh", "dir/a.sh"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "a/c"));
        assert!(!glob_match("a?c", "ac"));
        assert!(glob_match("src/*/x.sh", "src/lib/x.sh"));
        assert!(!glob_match("src/*/x.sh", "src/a/b/x.sh"));
        assert!(glob_match("src/**/x.sh", "src/a/b/x.sh"));
        assert!(glob_match("src/**/x.sh", "src/x.sh"));
        assert!(glob_match("**/x.sh", "x.sh"));
        assert!(glob_match("src/**", "src/a/b"));
        assert!(!glob_match("src/*", "src/a/b"));
        assert!(glob_match("*", ""));
        assert!(!glob_match("", "a"));
    }

    #[test]
    fn exclusion_forms() {
        assert!(is_excluded(&["node_modules"], "a/node_modules/b/c.sh"));
        assert!(is_excluded(&["*.generated.sh"], "x/y/foo.generated.sh"));
        assert!(!is_excluded(&["*.generated.sh"], "x/y/foo.sh"));
        assert!(is_excluded(&["vendor/lib"], "./vendor/lib/a.sh"));
        assert!(!is_excluded(&["vendor/lib"], "other/vendor/lib/a.sh"));
        assert!(is_excluded(&["**/lib"], "a/b/lib/c.sh"));
        assert!(is_excluded(&["build/"], "build/x.sh"));
        assert!(!is_excluded(&["build"], "."));
        assert!(!is_excluded(&[], "a.sh"));
    }

    #[test]
    fn shebangs() {
        for yes in [
            "#!/bin/sh\n",
            "#!/bin/bash -e\n",
            "#!/usr/bin/env bash\n",
            "#!/usr/bin/env -S bash -e\n",
            "#!/usr/bin/env FOO=1 dash\n",
            "#! /bin/ksh\n",
            "#!/bin/busybox sh\n",
            "#!/usr/bin/env -u X sh\n",
        ] {
            assert!(shebang_is_shell(yes.as_bytes()), "{yes:?}");
        }
        for no in ["#!/usr/bin/python\n", "#!/usr/bin/env python3\n", "echo hi\n", "", "#!\n", "#!/usr/bin/env\n"] {
            assert!(!shebang_is_shell(no.as_bytes()), "{no:?}");
        }
        // Only the first line counts.
        assert!(!shebang_is_shell(b"\n#!/bin/sh\n"));
    }

    #[test]
    fn hunks() {
        let d = "diff --git a/x b/x\n@@ -1,2 +1,3 @@\n+a\n@@ -5 +7 @@\n@@ -9,2 +10,0 @@\n@@ -20,0 +21,2 @@\n";
        assert_eq!(parse_hunks(d), vec![(1, 3), (7, 7), (21, 22)]);
    }
}
