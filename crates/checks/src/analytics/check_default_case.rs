// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkDefaultCase`, lines 4303-4315 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    PseudoGlob, pseudo_glob_is_superset_of, word_to_exact_pseudo_glob,
};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkDefaultCase",
    registry: "optional-tree",
    order: 3,
    optional: Some(StaticCheckDescription {
        name: "add-default-case",
        id: &[2249],
        description: "Suggest adding a default case in `case` statements",
        positive: "case $? in 0) echo 'Success';; esac",
        negative: "case $? in 0) echo 'Success';; *) echo 'Fail' ;; esac",
    }),
    kind: CheckKind::Node(check_default_case),
};

fn check_default_case(p: &Params<'_>, t: Id, c: &mut Comments) {
    // `T_CaseExpression id _ list`: `id` is the case expression's own id
    // (`t`), not the scrutinee word the Rust variant holds first.
    if let Token::T_CaseExpression(_, list) = p.ast.get(t) {
        if !list
            .iter()
            .any(|(_, pats, _)| pats.iter().any(|&pat| can_match_any_(p, pat)))
        {
            c.info(
                t,
                2249,
                "Consider adding a default *) case, even if it just exits with error.",
            );
        }
    }
}

fn can_match_any_(p: &Params<'_>, pat: Id) -> bool {
    match word_to_exact_pseudo_glob(p.ast, pat) {
        Some(pg) => pseudo_glob_is_superset_of(&pg, &[PseudoGlob::PGMany]),
        None => false,
    }
}
