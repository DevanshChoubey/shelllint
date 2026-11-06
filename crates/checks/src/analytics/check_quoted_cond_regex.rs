// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkQuotedCondRegex`, lines 1429-1445 (e11f964)

use std::sync::LazyLock;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;
use regex::Regex;
use shellcheck_core::regex::{matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkQuotedCondRegex",
    registry: "node",
    order: 26,
    optional: None,
    kind: CheckKind::Node(check_quoted_cond_regex),
};

// DIVERGENCE: POSIX class `[][*.+()|]` (leading `]`) is written with escapes for the regex crate.
static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex(r"[\]\[*.+()|]"));

fn check_quoted_cond_regex(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Binary(_, op, _, rhs) = p.ast.get(t) else { return };
    if op != "=~" {
        return;
    }
    let rhs = *rhs;
    if let Token::T_NormalWord(l) = p.ast.get(rhs) {
        if let &[x] = &l[..] {
            if matches!(p.ast.get(x), Token::T_DoubleQuoted(_) | Token::T_SingleQuoted(_)) {
                error(p, rhs, c);
            }
        }
    }
}

fn error(p: &Params<'_>, t: Id, c: &mut Comments) {
    if !is_constant_non_re(p, t) {
        c.warn(
            t,
            2076,
            "Remove quotes from right-hand side of =~ to match as a regex rather than literally.",
        );
    }
}

fn has_metachars(s: &str) -> bool {
    matches(s, &RE)
}

fn is_constant_non_re(p: &Params<'_>, t: Id) -> bool {
    match get_literal_string(p.ast, t) {
        Some(s) => !has_metachars(&s),
        None => false,
    }
}
