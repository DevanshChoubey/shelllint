// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkNullaryExpansionTest`, lines 4256-4272 (e11f964)

use crate::support::{fix_with, replace_start};
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_word_parts, is_command_substitution, is_constant};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkNullaryExpansionTest",
    registry: "optional-tree",
    order: 1,
    optional: Some(StaticCheckDescription {
        name: "avoid-nullary-conditions",
        id: &[2243, 2244],
        description: "Suggest explicitly using -n in `[ $var ]`",
        positive: "[ \"$var\" ] && [ \"$(cmd)\" ]",
        negative: "[ -n \"$var\" ] && [ -n \"$(cmd)\" ]",
    }),
    kind: CheckKind::Node(check_nullary_expansion_test),
};

fn check_nullary_expansion_test(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Nullary(_, word) = p.ast.get(t) else { return };
    let word = *word;
    let parts = get_word_parts(p.ast, word);
    let id = word;
    let fix = || fix_with(vec![replace_start(id, p, 0, "-n ")]);
    if parts.len() == 1 && is_command_substitution(p.ast, parts[0]) {
        c.style_with_fix(
            id,
            2243,
            "Prefer explicit -n to check for output (or run command without [/[[ to check for success).",
            fix(),
        );
    // If they're constant, you get SC2157 &co
    } else if parts.iter().all(|&x| !is_constant(p.ast, x)) {
        c.style_with_fix(
            id,
            2244,
            "Prefer explicit -n to check non-empty string (or use =/-ne to check boolean/integer).",
            fix(),
        );
    }
}
