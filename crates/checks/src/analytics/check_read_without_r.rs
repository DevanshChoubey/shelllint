// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkReadWithoutR`, lines 3358-3369 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_unqualified_command};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    arguments, get_all_flags, get_command_token_or_this, get_gnu_opts, get_literal_string,
};
use shellcheck_core::data::FLAGS_FOR_READ;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkReadWithoutR",
    registry: "node",
    order: 65,
    optional: None,
    kind: CheckKind::Node(check_read_without_r),
};

fn check_read_without_r(p: &Params<'_>, t: Id, c: &mut Comments) {
    if !matches!(p.ast.get(t), Token::T_SimpleCommand(..)) {
        return;
    }
    if !is_unqualified_command(p.ast, t, "read") {
        return;
    }
    let flags = get_all_flags(p.ast, t);
    if flags.iter().any(|(_, s)| s == "r") {
        return;
    }
    let has_t0 = || -> bool {
        let Some(parsed) = get_gnu_opts(p.ast, FLAGS_FOR_READ, &arguments(p.ast, t)) else {
            return false;
        };
        let Some((_, (_, v))) = parsed.iter().find(|(n, _)| n == "t") else {
            return false;
        };
        get_literal_string(p.ast, *v).as_deref() == Some("0")
    };
    if has_t0() {
        return;
    }
    c.info(get_command_token_or_this(p.ast, t), 2162, "read without -r will mangle backslashes.");
}
