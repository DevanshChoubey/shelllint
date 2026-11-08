// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSplittingInArrays`, lines 3766-3794 (e11f964)

use shelllint_analyzer::analyzerlib::{
    Comments, Params, is_counting_reference, is_quoted_alternative_reference,
};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_braced_reference, oversimplify};
use shelllint_core::data::VARIABLES_WITHOUT_SPACES;
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSplittingInArrays",
    registry: "node",
    order: 73,
    optional: None,
    kind: CheckKind::Node(check_splitting_in_arrays),
};

fn check_splitting_in_arrays(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Array(elements) = p.ast.get(t) else { return };
    for &word in elements {
        if let Token::T_NormalWord(parts) = p.ast.get(word) {
            for &part in parts {
                check_part(p, part, c);
            }
        }
    }
}

fn check_part(p: &Params<'_>, part: Id, c: &mut Comments) {
    match p.ast.get(part) {
        Token::T_DollarExpansion(_)
        | Token::T_DollarBraceCommandExpansion(_, _)
        | Token::T_Backticked(_) => for_command(p, part, c),
        Token::T_DollarBraced(_, s) => {
            let s = *s;
            if !is_counting_reference(p.ast, part)
                && !is_quoted_alternative_reference(p.ast, part)
                && !VARIABLES_WITHOUT_SPACES
                    .contains(&get_braced_reference(&oversimplify(p.ast, s).concat()).as_str())
            {
                c.warn(
                    part,
                    2206,
                    if p.shell_type == Shell::Ksh {
                        "Quote to prevent word splitting/globbing, or split robustly with read -A or while read."
                    } else {
                        "Quote to prevent word splitting/globbing, or split robustly with mapfile or read -a."
                    },
                );
            }
        }
        _ => {}
    }
}

fn for_command(p: &Params<'_>, id: Id, c: &mut Comments) {
    c.warn(
        id,
        2207,
        if p.shell_type == Shell::Ksh {
            "Prefer read -A or while read to split command output (or quote to avoid splitting)."
        } else {
            "Prefer mapfile or read -a to split command output (or quote to avoid splitting)."
        },
    );
}
