// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkCatastrophicRm`, lines 1157-1206 (e11f964)

use std::collections::HashSet;
use std::sync::LazyLock;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::prelude::please_report;
use shellcheck_core::astlib::{
    arguments, get_all_flags, get_literal_string_ext, only_literal_string,
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
        for arg in arguments(p.ast, t) {
            for parts in brace_expand_parts(p.ast, arg) {
                check_word(p.ast, arg, &parts, c);
            }
        }
    }
}

/// `braceExpand word`, with each result word given as the list of the
/// original leaf parts it concatenates.
///
/// Every `braceExpand` result is `T_NormalWord id parts` whose parts are
/// original tokens or, for nested braces, further such words. The checks
/// below only read results through `getLiteralStringExt`, which maps a
/// `T_NormalWord` to the concatenation of its parts, so a result reads the
/// same as its flattened part list. This avoids allocating the result words
/// in a copy of the arena (an O(script) clone for every recursive `rm`). The
/// 1000-result caps are those of `astlib::brace_expand`, at the same points.
fn brace_expand_parts(ast: &Ast, id: Id) -> Vec<Vec<Id>> {
    let Token::T_NormalWord(list) = ast.get(id) else {
        panic!("{}", please_report("braceExpand on non-NormalWord"));
    };
    let mut options: Vec<Vec<Vec<Id>>> = Vec::new();
    for &p in list {
        match ast.get(p) {
            Token::T_BraceExpansion(items) => {
                let mut o = Vec::new();
                for &item in items {
                    o.extend(brace_expand_parts(ast, item));
                    if o.len() >= 1000 {
                        break;
                    }
                }
                o.truncate(1000);
                options.push(o);
            }
            _ => options.push(vec![vec![p]]),
        }
    }
    let mut combos: Vec<Vec<Id>> = vec![vec![]];
    for o in options {
        let mut next = Vec::new();
        'outer: for c in &combos {
            for x in &o {
                let mut n = c.clone();
                n.extend_from_slice(x);
                next.push(n);
                if next.len() >= 1000 {
                    break 'outer;
                }
            }
        }
        combos = next;
    }
    combos.truncate(1000);
    combos
}

/// `getLiteralStringExt more` on a brace expansion result of `word`.
fn literal_of_parts(ast: &Ast, more: &dyn Fn(Id) -> Option<String>, parts: &[Id]) -> Option<String> {
    let mut out = String::new();
    for &p in parts {
        out.push_str(&get_literal_string_ext(ast, more, p)?);
    }
    Some(out)
}

/// `checkWord token`; `token` is a result of `braceExpand word`, which
/// carries `word`'s id.
fn check_word(ast: &Ast, word: Id, token: &[Id], c: &mut Comments) {
    // getGlobOrLiteralString = getLiteralStringExt f
    //   where f (T_Glob _ str) = return str; f _ = Nothing
    let glob = |x: Id| match ast.get(x) {
        Token::T_Glob(str) => Some(str.clone()),
        _ => None,
    };
    match literal_of_parts(ast, &glob, token) {
        Some(str) => {
            if is_important_path(&fix_path(&str)) {
                c.warn(word, 2114, "Warning: deletes a system directory.");
            }
        }
        None => check_word2(ast, word, token, c),
    }
}

fn check_word2(ast: &Ast, word: Id, token: &[Id], c: &mut Comments) {
    let Some(filename) = get_potential_path(ast, token) else { return };
    let path = fix_path(&filename);
    if is_important_path(&path) {
        c.warn(
            word,
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

fn get_potential_path(ast: &Ast, token: &[Id]) -> Option<String> {
    literal_of_parts(
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

fn is_important_path(s: &str) -> bool {
    static IMPORTANT_PATHS: LazyLock<HashSet<String>> = LazyLock::new(|| important_paths().into_iter().collect());
    IMPORTANT_PATHS.contains(s)
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
