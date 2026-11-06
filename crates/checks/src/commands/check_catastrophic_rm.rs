// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkCatastrophicRm`, lines 1157-1206 (e11f964)

use crate::support::Scratch;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    arguments, get_all_flags, get_glob_or_literal_string, get_literal_string_ext, only_literal_string,
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkCatastrophicRm",
    registry: "command",
    order: 30,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("rm"), check_catastrophic_rm),
};

fn check_catastrophic_rm(p: &Params<'_>, t: Id, c: &mut Comments) {
    let is_recursive = get_all_flags(p.ast, t)
        .iter()
        .any(|(_, f)| f == "r" || f == "R" || f == "recursive");
    if is_recursive {
        let mut s = Scratch::new(p.ast);
        for arg in arguments(p.ast, t) {
            for token in s.brace_expand(arg) {
                check_word(&s, token, c);
            }
        }
    }
}

fn check_word(s: &Scratch, token: Id, c: &mut Comments) {
    match get_glob_or_literal_string(&s.ast, token) {
        Some(str) => {
            if important_paths().contains(&fix_path(&str)) {
                c.warn(s.haskell_id(token), 2114, "Warning: deletes a system directory.");
            }
        }
        None => check_word2(s, token, c),
    }
}

fn check_word2(s: &Scratch, token: Id, c: &mut Comments) {
    let Some(filename) = get_potential_path(s, token) else { return };
    let path = fix_path(&filename);
    if important_paths().contains(&path) {
        c.warn(
            s.haskell_id(token),
            2115,
            &format!("Use \"${{var:?}}\" to ensure this never expands to {} .", path),
        );
    }
}

fn fix_path(filename: &str) -> String {
    let normalized = skip_repeating('/', &skip_repeating('*', filename));
    if normalized == "/" {
        normalized
    } else {
        strip_trailing('/', &normalized)
    }
}

fn get_potential_path(s: &Scratch, token: Id) -> Option<String> {
    let ast = &s.ast;
    get_literal_string_ext(
        ast,
        &|x| match ast.get(x) {
            Token::T_Glob(str) => Some(str.clone()),
            Token::T_DollarBraced(_, word) => {
                let var = only_literal_string(ast, *word);
                // This shouldn't handle non-colon cases.
                if [":?", ":-", ":="].iter().any(|n| var.contains(n)) {
                    None
                } else {
                    Some(String::new())
                }
            }
            _ => Some(String::new()),
        },
        token,
    )
}

fn strip_trailing(c: char, s: &str) -> String {
    s.trim_end_matches(c).to_string()
}

/// `foldr go []`: collapse runs of `c` into a single `c`.
fn skip_repeating(c: char, s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev: Option<char> = None;
    for a in s.chars() {
        if !(a == c && prev == Some(c)) {
            out.push(a);
        }
        prev = Some(a);
    }
    out
}

fn important_paths() -> Vec<String> {
    let paths = [
        "", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/media", "/mnt", "/opt", "/proc", "/root",
        "/run", "/sbin", "/srv", "/sys", "/usr", "/usr/bin", "/usr/include", "/usr/lib", "/usr/libexec",
        "/usr/lib64", "/usr/local", "/usr/sbin", "/usr/share", "/var",
    ];
    let mut out = Vec::new();
    for x in ["", "/", "/*", "/*/*"] {
        for p in paths {
            let s = format!("{p}{x}");
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    out
}
