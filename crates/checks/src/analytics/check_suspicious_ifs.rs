// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSuspiciousIFS`, lines 3197-3215 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_literal_string;
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSuspiciousIFS",
    registry: "node",
    order: 60,
    optional: None,
    kind: CheckKind::Node(check_suspicious_ifs),
};

fn check_suspicious_ifs(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Assignment(_, name, indices, value) = p.ast.get(t) else { return };
    if name != "IFS" || !indices.is_empty() {
        return;
    }
    let value = *value;
    let Some(s) = get_literal_string(p.ast, value) else { return };

    let has_dollar_single = p.shell_type == Shell::Bash || p.shell_type == Shell::Ksh;
    let n = if has_dollar_single { "$'\\n'" } else { "'<literal linefeed here>'" };
    let tab = if has_dollar_single { "$'\\t'" } else { "\"$(printf '\\t')\"" };

    let suggest = |c: &mut Comments, r: &str| {
        c.warn(value, 2141, &format!("This backslash is literal. Did you mean IFS={} ?", r))
    };
    let suggest2 = |c: &mut Comments, desc: &str| {
        c.warn(
            value,
            2141,
            &format!(
                "This IFS value contains {}. For tabs/linefeeds/escapes, use $'..', literal, or printf.",
                desc
            ),
        )
    };

    if s == "\\n" {
        suggest(c, n)
    } else if s == "\\t" {
        suggest(c, tab)
    } else if s.contains('\\') {
        suggest2(c, "a literal backslash")
    } else if s.contains('n') {
        suggest2(c, "the literal letter 'n'")
    } else if s.contains('t') {
        suggest2(c, "the literal letter 't'")
    }
}
