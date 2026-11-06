// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkGrepRe`, lines 362-406 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params, is_confused_glob_regex};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{
    arguments, get_all_flags, get_literal_string_def, is_glob, oversimplify,
};
use shellcheck_core::data::SAMPLE_WORDS;
use shellcheck_core::prelude::head_or_default;
use shellcheck_core::regex::{match_regex, matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkGrepRe",
    registry: "command",
    order: 3,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("grep"), check_grep_re),
};

const GREP_GLOB_FLAGS: &[&str] = &[
    "fixed-strings", "F", "include", "exclude", "exclude-dir", "o", "only-matching",
];

fn check_grep_re(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    let args = arguments(p.ast, cmd);
    f(p, cmd, &args, c);
}

// --regex=*(extglob) doesn't work. Fixme?
fn skippable(s: &str) -> bool {
    !s.starts_with("--regex=") && s.starts_with('-')
}

fn f(p: &Params<'_>, cmd: Id, args: &[Id], c: &mut Comments) {
    let mut rest = args;
    while let Some((&x, r)) = rest.split_first() {
        let str = get_literal_string_def(p.ast, "_", x);
        if ["--", "-e", "--regex"].contains(&str.as_str()) {
            // Regex is *after* this
            check_re(p, cmd, r, c);
            return;
        } else if skippable(&str) {
            // Regex is elsewhere
            rest = r;
        } else {
            // Regex is this
            check_re(p, cmd, rest, c);
            return;
        }
    }
}

fn check_re(p: &Params<'_>, cmd: Id, args: &[Id], c: &mut Comments) {
    let Some(&re) = args.first() else { return };
    if is_glob(p.ast, re) {
        c.warn(re, 2062, "Quote the grep pattern so the shell won't interpret it.");
    }

    let flags: Vec<String> = get_all_flags(p.ast, cmd).into_iter().map(|(_, s)| s).collect();
    if !GREP_GLOB_FLAGS.iter().any(|g| flags.iter().any(|f| f == g)) {
        let string: String = oversimplify(p.ast, re).concat();
        if is_confused_glob_regex(&string) {
            c.warn(re, 2063, "Grep uses regex, but this looks like a glob.");
        } else if let Some(ch) = get_suspicious_regex_wildcard(&string) {
            c.info(
                re,
                2022,
                format!(
                    "Note that unlike globs, {ch}* here matches '{ch}{ch}{ch}' but not '{}'.",
                    word_starting_with(ch)
                ),
            );
        }
    }
}

fn word_starting_with(ch: char) -> String {
    let upper = |w: &&str| {
        let mut cs = w.chars();
        let first = cs.next().expect("sample words are non-empty");
        first.to_uppercase().chain(cs).collect::<String>()
    };
    let candidates: Vec<String> = SAMPLE_WORDS
        .iter()
        .map(|w| w.to_string())
        .chain(SAMPLE_WORDS.iter().map(upper))
        .collect();
    let matching: Vec<String> =
        candidates.into_iter().filter(|w| w.starts_with(ch)).collect();
    head_or_default(format!("{ch}test"), &matching)
}

fn get_suspicious_regex_wildcard(str: &str) -> Option<char> {
    match match_regex(&SUSPICIOUS, str) {
        Some(g) if g.len() == 1 && g[0].chars().count() == 1 && !matches(str, &CONTRA) => {
            g[0].chars().next()
        }
        _ => None,
    }
}

static SUSPICIOUS: LazyLock<Regex> = LazyLock::new(|| mk_regex("([A-Za-z1-9])\\*"));
// DIVERGENCE: POSIX `[][^$+\\]` (leading `]` literal) is written with escaped
// brackets for the Rust regex crate; same character set.
static CONTRA: LazyLock<Regex> =
    LazyLock::new(|| mk_regex("[^a-zA-Z1-9]\\*|[\\]\\[^$+\\\\]"));
