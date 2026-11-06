// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnquotedN`, lines 1238-1242 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::{get_word_parts, is_array_expansion, will_split};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnquotedN",
    registry: "node",
    order: 7,
    optional: None,
    kind: CheckKind::Node(check_unquoted_n),
};

fn check_unquoted_n(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Unary(ConditionType::SingleBracket, op, x) = p.ast.get(t) else { return };
    let x = *x;
    if op == "-n" && will_split(p.ast, x) {
        // There's SC2198 for these
        if !get_word_parts(p.ast, x).into_iter().any(|w| is_array_expansion(p.ast, w)) {
            c.err(x, 2070, "-n doesn't work with unquoted arguments. Quote or use [[ ]].");
        }
    }
}
