// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkVariableBraces`, lines 2352-2362 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_reference, oversimplify};
use shellcheck_core::data::UNBRACED_VARIABLES;

use crate::support::{fix_with, quotes_may_conflict_with_sc2281, replace_end, replace_start};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkVariableBraces",
    registry: "optional-tree",
    order: 4,
    optional: Some(StaticCheckDescription {
        name: "require-variable-braces",
        id: &[2250],
        description: "Suggest putting braces around all variable references",
        positive: "var=hello; echo $var",
        negative: "var=hello; echo ${var}",
    }),
    kind: CheckKind::Node(check_variable_braces),
};

fn check_variable_braces(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_DollarBraced(false, l) = params.ast.get(t) else { return };
    let name = get_braced_reference(&oversimplify(params.ast, *l).concat());
    if !UNBRACED_VARIABLES.contains(&name.as_str()) && !quotes_may_conflict_with_sc2281(params, t) {
        c.style_with_fix(
            t,
            2250,
            "Prefer putting braces around variable references even when not strictly required.",
            fix_for(params, t),
        );
    }
}

fn fix_for(params: &Params<'_>, token: Id) -> shellcheck_core::interface::Fix {
    fix_with(vec![replace_start(token, params, 1, "${"), replace_end(token, params, 0, "}")])
}
