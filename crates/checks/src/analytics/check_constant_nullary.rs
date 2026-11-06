// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkConstantNullary`, lines 1528-1539 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{is_constant, only_literal_string};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkConstantNullary",
    registry: "node",
    order: 12,
    optional: None,
    kind: CheckKind::Node(check_constant_nullary),
};

fn check_constant_nullary(params: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TC_Nullary(_, t) = params.ast.get(t) else { return };
    let t = *t;
    if !is_constant(params.ast, t) {
        return;
    }
    match only_literal_string(params.ast, t).as_str() {
        "false" => c.err(t, 2158, "[ false ] is true. Remove the brackets."),
        "0" => c.err(t, 2159, "[ 0 ] is true. Use 'false' instead."),
        "true" => c.style(t, 2160, "Instead of '[ true ]', just use 'true'."),
        "1" => c.style(t, 2161, "Instead of '[ 1 ]', use 'true'."),
        _ => c.err(t, 2078, "This expression is constant. Did you forget a $ somewhere?"),
    }
}
