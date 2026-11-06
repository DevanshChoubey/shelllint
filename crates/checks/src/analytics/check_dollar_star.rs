// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDollarStar`, lines 954-964 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_strictly_quote_free};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, is_variable_char, oversimplify};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDollarStar",
    registry: "node",
    order: 4,
    optional: None,
    kind: CheckKind::Node(check_dollar_star),
};

fn check_dollar_star(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(t) else { return };
    let &[part] = &parts[..] else { return };
    let Token::T_DollarBraced(_, l) = p.ast.get(part) else { return };
    if !is_strictly_quote_free(p.ast, p.shell_type, t) {
        let str = oversimplify(p.ast, *l).concat();
        if str.starts_with('*') {
            c.warn(part, 2048, "Use \"$@\" (with quotes) to prevent whitespace problems.");
        }
        if get_braced_modifier(&str).starts_with("[*]")
            && is_variable_char(str.chars().next().unwrap_or('!'))
        {
            c.warn(part, 2048, "Use \"${array[@]}\" (with quotes) to prevent whitespace problems.");
        }
    }
}
