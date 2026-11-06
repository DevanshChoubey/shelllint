// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRequireDoubleBracket`, lines 4864-4895 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::interface::{Shell, TokenComment};

use crate::support::{fix_with, node_checks_to_tree_check, replace_end, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRequireDoubleBracket",
    registry: "optional-tree",
    order: 6,
    optional: Some(StaticCheckDescription {
        name: "require-double-brackets",
        id: &[2292],
        description: "Require [[ and warn about [ in Bash/Ksh",
        positive: "[ -e /etc/issue ]",
        negative: "[[ -e /etc/issue ]]",
    }),
    kind: CheckKind::Tree(check_require_double_bracket),
};

fn check_require_double_bracket(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    if matches!(params.shell_type, Shell::Bash | Shell::Ksh | Shell::BusyboxSh) {
        node_checks_to_tree_check(params, root, &[check])
    } else {
        vec![]
    }
}

fn check(params: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Condition(ConditionType::SingleBracket, _, _) = params.ast.get(t) {
        c.style_with_fix(
            t,
            2292,
            "Prefer [[ ]] over [ ] for tests in Bash/Ksh/Busybox.",
            fix_for(params, t),
        );
    }
}

fn fix_for(params: &Params<'_>, t: Id) -> shellcheck_core::interface::Fix {
    let mut v = vec![];
    if is_simple(params, t) {
        v.push(replace_start(t, params, 0, "["));
        // Don't add another ] if it was (wrongly) closed with ]] already
        if !is_closed_with_double(params, t) {
            v.push(replace_end(t, params, 0, "]"));
        }
    }
    fix_with(v)
}

fn is_closed_with_double(params: &Params<'_>, t: Id) -> bool {
    matches!(params.ast.get(t), Token::T_Condition(_, ConditionType::DoubleBracket, _))
}

// We don't tag operators like < and -o well enough to replace them,
// so just handle the simple cases.
fn is_simple(params: &Params<'_>, t: Id) -> bool {
    match params.ast.get(t) {
        Token::T_Condition(_, _, s) => is_simple(params, *s),
        Token::TC_Binary(_, op, _, _) => !op.chars().any(|x| x == '<' || x == '>'),
        Token::TC_Unary(..) => true,
        Token::TC_Nullary(..) => true,
        _ => false,
    }
}
