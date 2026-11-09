// SPDX-License-Identifier: GPL-3.0-or-later
// Not a port: helpers for the configuration features this project adds on top
// of ShellCheck (`severity=`, `~`/`$HOME` expansion, `external-vars=` and the
// nix-shell shebang). Every use site carries a `// DIVERGENCE:` comment.

use crate::ast::{Annotation, Ast, Id, Token};
use crate::data::shell_for_executable;
use crate::interface::Severity;

/// The environment variable that names the home directory.
pub const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

/// `expand_home_with` with the home directory taken from the environment.
pub fn expand_home(s: &str) -> String {
    expand_home_with(s, std::env::var(HOME_VAR).ok().as_deref())
}

/// Expands a leading `~`, `$HOME` or `${HOME}` (only as a whole path
/// component, so `~user` and `$HOMEDIR` are left alone). With no home
/// directory (`None` or empty) the text is returned unchanged.
pub fn expand_home_with(s: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|h| !h.is_empty()) else {
        return s.to_string();
    };
    for prefix in ["${HOME}", "$HOME", "~"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with('/') {
                return format!("{home}{rest}");
            }
        }
    }
    s.to_string()
}

/// Is `name` a valid `external-vars` entry: a variable name with an optional
/// trailing `*`?
pub fn is_external_var_pattern(name: &str) -> bool {
    let base = name.strip_suffix('*').unwrap_or(name);
    let mut chars = base.chars();
    let first_ok = match chars.next() {
        Some(c) => c.is_ascii_alphabetic() || c == '_',
        // A bare `*` matches everything; the empty string is not a name.
        None => name == "*",
    };
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Does `var` match one `external-vars` entry? A trailing `*` makes the entry
/// a prefix glob.
pub fn external_var_matches(pattern: &str, var: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => var.starts_with(prefix),
        None => pattern == var,
    }
}

/// The first `severity=` annotation of a script's root token (file-level
/// directives come before rc-file ones).
pub fn get_severity_directive(ast: &Ast, root: Id) -> Option<Severity> {
    match ast.get(root) {
        Token::T_Annotation(list, _) => list.iter().find_map(|a| match a {
            Annotation::MinSeverity(s) => Some(*s),
            _ => None,
        }),
        _ => None,
    }
}

/// Every `external-vars` name in a `T_Annotation` anywhere under `root`.
pub fn get_external_var_directives(ast: &Ast, root: Id) -> Vec<String> {
    let mut out = vec![];
    ast.analyze(
        root,
        &mut |id| {
            if let Token::T_Annotation(list, _) = ast.get(id) {
                for a in list {
                    if let Annotation::ExternalVar(n) = a {
                        out.push(n.clone());
                    }
                }
            }
        },
        &mut |_| {},
    );
    out
}

/// Splits `s` into words like a shell would for plain and quoted text: runs
/// of blanks separate words, and `'...'` or `"..."` group (without escapes).
fn split_quoted_words(s: &str) -> Vec<String> {
    let mut words = vec![];
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// The shell a nix-shell script runs under. `lines` are the contents (after
/// the `#!`) of the run of `#!` lines that follows the nix-shell shebang.
/// `#! nix-shell -i <interpreter>` names the interpreter (the first word of
/// it); with no `-i` nix-shell runs bash. `None` when the interpreter is not
/// a shell ShellCheck supports (such a script keeps its normal shebang
/// handling).
pub fn nix_shell_interpreter(lines: &[String]) -> Option<String> {
    let mut interpreter: Option<String> = None;
    for line in lines {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("nix-shell") else { continue };
        if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace()) {
            continue;
        }
        let words = split_quoted_words(rest);
        let mut it = words.iter();
        while let Some(w) = it.next() {
            let value = if w == "-i" {
                it.next().cloned()
            } else {
                w.strip_prefix("-i").filter(|v| !v.is_empty() && !v.starts_with('-')).map(String::from)
            };
            if let Some(v) = value {
                interpreter = Some(v);
            }
        }
        if interpreter.is_some() {
            break;
        }
    }
    let Some(spec) = interpreter else { return Some("bash".to_string()) };
    let first = spec.split_whitespace().next()?;
    let name = first.rsplit('/').next().unwrap_or(first);
    shell_for_executable(name).map(|_| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_forms() {
        let h = Some("/home/u");
        assert_eq!(expand_home_with("~", h), "/home/u");
        assert_eq!(expand_home_with("~/lib", h), "/home/u/lib");
        assert_eq!(expand_home_with("$HOME/lib", h), "/home/u/lib");
        assert_eq!(expand_home_with("${HOME}/lib", h), "/home/u/lib");
        assert_eq!(expand_home_with("${HOME}", h), "/home/u");
    }

    #[test]
    fn home_left_alone() {
        let h = Some("/home/u");
        assert_eq!(expand_home_with("~bob/lib", h), "~bob/lib");
        assert_eq!(expand_home_with("$HOMEDIR/x", h), "$HOMEDIR/x");
        assert_eq!(expand_home_with("a/~/b", h), "a/~/b");
        assert_eq!(expand_home_with("SCRIPTDIR/x", h), "SCRIPTDIR/x");
    }

    #[test]
    fn home_unset_or_empty() {
        assert_eq!(expand_home_with("~/lib", None), "~/lib");
        assert_eq!(expand_home_with("$HOME/lib", Some("")), "$HOME/lib");
    }

    #[test]
    fn external_var_patterns() {
        assert!(external_var_matches("CI", "CI"));
        assert!(!external_var_matches("CI", "CI_X"));
        assert!(external_var_matches("CI_*", "CI_X"));
        assert!(external_var_matches("CI_*", "CI_"));
        assert!(!external_var_matches("CI_*", "CI"));
        assert!(!external_var_matches("CI_*", "XCI_A"));
        assert!(is_external_var_pattern("CI_*"));
        assert!(is_external_var_pattern("_a1"));
        assert!(!is_external_var_pattern("1a"));
        assert!(!is_external_var_pattern("a-b"));
        assert!(!is_external_var_pattern(""));
        assert!(!is_external_var_pattern("a*b"));
    }

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nix_interpreters() {
        let n = |l: &[&str]| nix_shell_interpreter(&lines(l));
        assert_eq!(n(&[]), Some("bash".into()));
        assert_eq!(n(&[" nix-shell -p git"]), Some("bash".into()));
        assert_eq!(n(&[" nix-shell -i dash -p git"]), Some("dash".into()));
        assert_eq!(n(&[" nix-shell -p git -i bash"]), Some("bash".into()));
        assert_eq!(n(&[" nix-shell -i \"bash -x\""]), Some("bash".into()));
        assert_eq!(n(&["nix-shell -i '/nix/store/x/bin/ksh'"]), Some("ksh".into()));
        assert_eq!(n(&[" nix-shell -ibash"]), Some("bash".into()));
        assert_eq!(n(&[" nix-shell -p git", " nix-shell -i dash"]), Some("dash".into()));
        assert_eq!(n(&[" nix-shell -i python3 -p python3"]), None);
        assert_eq!(n(&[" nix-shell -i runghc"]), None);
        assert_eq!(n(&[" not-nix-shell -i python"]), Some("bash".into()));
    }
}
