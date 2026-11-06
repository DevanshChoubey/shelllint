// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSingleBracketOperators`, lines 1388-1392 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSingleBracketOperators",
    registry: "node",
    order: 9,
    optional: None,
    kind: CheckKind::Node(check_single_bracket_operators),
};

fn check_single_bracket_operators(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::TC_Binary(ConditionType::SingleBracket, op, _, _) = p.ast.get(t) {
        if op == "=~" && matches!(p.shell_type, Shell::Bash | Shell::Ksh) {
            c.err(t, 2074, "Can't use =~ in [ ]. Use [[..]] instead.");
        }
    }
}
