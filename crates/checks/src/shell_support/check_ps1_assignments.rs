// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkPS1Assignments`, lines 673-689 (e11f964)

use std::sync::LazyLock;

use regex::Regex;

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::oversimplify;
use shelllint_core::interface::Shell;
use shelllint_core::regex::{match_regex, mk_regex, sub_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkPS1Assignments",
    registry: "shell-support",
    order: 5,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash], f),
};

static ENCLOSED_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("\\\\\\[.*\\\\\\]")); // FIXME: shouldn't be eager
static ESCAPE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| mk_regex("\\\\x1[Bb]|\\\\e|\x1B|\\\\033"));

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Assignment(_, name, _, word) = p.ast.get(t) {
        if name == "PS1" {
            warn_for(p, *word, c);
        }
    }
}

fn warn_for(p: &Params<'_>, word: Id, c: &mut Comments) {
    let contents = oversimplify(p.ast, word).concat();
    if contains_unescaped(&contents) {
        c.info(
            word,
            2025,
            "Make sure all escape sequences are enclosed in \\[..\\] to prevent line wrapping issues",
        );
    }
}

fn contains_unescaped(s: &str) -> bool {
    let unenclosed = sub_regex(&ENCLOSED_REGEX, s, "");
    match_regex(&ESCAPE_REGEX, &unenclosed).is_some()
}
