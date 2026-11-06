// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkShebang`, lines 700-717 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Annotation, Id, Token};
use shellcheck_core::astlib::executable_from_shebang;
use shellcheck_core::interface::TokenComment;
use shellcheck_core::unicode::words;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkShebang",
    registry: "tree",
    order: 7,
    optional: None,
    kind: CheckKind::Tree(check_shebang),
};

fn check_shebang(params: &Params<'_>, t: Id) -> Vec<TokenComment> {
    match params.ast.get(t) {
        Token::T_Annotation(list, t) => {
            let is_override = |a: &Annotation| matches!(a, Annotation::ShellOverride(_));
            if list.iter().any(is_override) {
                vec![]
            } else {
                check_shebang(params, *t)
            }
        }
        Token::T_Script(id, _) => {
            let id = *id;
            let Token::T_Literal(sb) = params.ast.get(id) else {
                return vec![];
            };
            let mut c = Comments::new();
            if !params.shell_type_specified {
                if sb.is_empty() {
                    c.err(
                        id,
                        2148,
                        "Tips depend on target shell and yours is unknown. Add a shebang or a 'shell' directive.",
                    );
                }
                if executable_from_shebang(sb) == "ash" {
                    c.warn(
                        id,
                        2187,
                        "Ash scripts will be checked as Dash. Add '# shellcheck shell=dash' to silence.",
                    );
                }
            }
            if !sb.is_empty() {
                if !sb.starts_with('/') {
                    c.err(
                        id,
                        2239,
                        "Ensure the shebang uses an absolute path to the interpreter.",
                    );
                }
                if words(sb).first().is_some_and(|w| w.ends_with('/')) {
                    c.err(
                        id,
                        2246,
                        "This shebang specifies a directory. Ensure the interpreter is a file.",
                    );
                }
            }
            c.into_vec()
        }
        _ => vec![],
    }
}
