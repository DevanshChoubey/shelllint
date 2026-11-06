// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkTrailingBracket`, lines 3491-3510 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::oversimplify;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkTrailingBracket",
    registry: "node",
    order: 68,
    optional: None,
    kind: CheckKind::Node(check_trailing_bracket),
};

fn check_trailing_bracket(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_SimpleCommand(_, tokens) if !tokens.is_empty() => {
            check(p, *tokens.last().unwrap(), t, c)
        }
        _ => {}
    }
}

fn check(p: &Params<'_>, last: Id, command: Id, c: &mut Comments) {
    let Token::T_NormalWord(parts) = p.ast.get(last) else { return };
    let &[part] = &parts[..] else { return };
    let Token::T_Literal(s) = p.ast.get(part) else { return };
    let str = s.as_str();
    if str == "]]" || str == "]" {
        let opposite = invert(str);
        let parameters = oversimplify(p.ast, command);
        if !parameters.iter().any(|x| x == opposite) {
            c.warn(
                last,
                2171,
                format!(
                    "Found trailing {} outside test. Add missing {} or quote if intentional.",
                    str, opposite
                ),
            );
        }
    }
}

fn invert(s: &str) -> &str {
    match s {
        "]]" => "[[",
        "]" => "[",
        x => x,
    }
}
