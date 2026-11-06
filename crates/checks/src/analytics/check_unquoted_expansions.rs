// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnquotedExpansions`, lines 842-857 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_quote_free, used_as_command_name};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_command_name_from_expansion;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnquotedExpansions",
    registry: "node",
    order: 46,
    optional: None,
    kind: CheckKind::Node(check_unquoted_expansions),
};

fn check_unquoted_expansions(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_DollarExpansion(cs) => examine(p, t, cs, c),
        Token::T_Backticked(cs) => examine(p, t, cs, c),
        Token::T_DollarBraceCommandExpansion(_, cs) => examine(p, t, cs, c),
        _ => {}
    }
}

fn examine(p: &Params<'_>, t: Id, contents: &[Id], c: &mut Comments) {
    if !(contents.is_empty()
        || should_be_split(p, t)
        || is_quote_free(p.ast, p.shell_type, t)
        || used_as_command_name(p.ast, t))
    {
        c.warn(t, 2046, "Quote this to prevent word splitting.");
    }
}

fn should_be_split(p: &Params<'_>, t: Id) -> bool {
    matches!(
        get_command_name_from_expansion(p.ast, t).as_deref(),
        Some("seq") | Some("pgrep")
    )
}
