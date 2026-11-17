// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDollarStar`, lines 954-964 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, is_strictly_quote_free};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_braced_modifier, is_variable_char, oversimplify};

use crate::zsh_support::{explicit_split, splits};

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
    // zsh: `$*` and `${a[*]}` are only re-split under SH_WORD_SPLIT or as `${=*}`.
    if !is_strictly_quote_free(p.ast, p.shell_type, t) && splits(p, part) {
        let str = oversimplify(p.ast, *l).concat();
        let str = if explicit_split(p, part) { str.replacen('=', "", 1) } else { str };
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
