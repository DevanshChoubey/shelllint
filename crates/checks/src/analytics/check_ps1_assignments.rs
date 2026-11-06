// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPS1Assignments`, lines 1850-1862 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;
use shellcheck_core::regex::{match_regex, mk_regex, sub_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPS1Assignments",
    registry: "node",
    order: 33,
    optional: None,
    kind: CheckKind::Node(check_ps1_assignments),
};

static ENCLOSED_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("\\\\\\[.*\\\\\\]")); // FIXME: shouldn't be eager
static ESCAPE_REGEX: LazyLock<Regex> = LazyLock::new(|| mk_regex("\\\\x1[Bb]|\\\\e|\x1B|\\\\033"));

fn check_ps1_assignments(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(_, name, _, word) = p.ast.get(t) else { return };
    if name != "PS1" {
        return;
    }
    warn_for(p, *word, c);
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
