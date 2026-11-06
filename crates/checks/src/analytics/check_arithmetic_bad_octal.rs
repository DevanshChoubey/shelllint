// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkArithmeticBadOctal`, lines 1597-1604 (e11f964)

use regex::Regex;
use std::sync::LazyLock;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;
use shellcheck_core::regex::{matches, mk_regex};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkArithmeticBadOctal",
    registry: "node",
    order: 15,
    optional: None,
    kind: CheckKind::Node(check_arithmetic_bad_octal),
};

fn check_arithmetic_bad_octal(p: &Params<'_>, t: Id, c: &mut Comments) {
    static OCTAL_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^0[0-7]*[8-9]"));
    let Token::TA_Expansion(_) = p.ast.get(t) else { return };
    let Some(s) = get_literal_string(p.ast, t) else { return };
    if matches(&s, &OCTAL_RE) {
        c.err(t, 2080, "Numbers with leading 0 are considered octal.");
    }
}
