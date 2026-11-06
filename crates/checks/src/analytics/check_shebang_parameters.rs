// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkShebangParameters`, lines 675-681 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Params, make_comment};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::interface::{Severity, TokenComment};
use shellcheck_core::regex::{matches, mk_regex};
use shellcheck_core::unicode::words;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkShebangParameters",
    registry: "tree",
    order: 2,
    optional: None,
    kind: CheckKind::Tree(check_shebang_parameters),
};

static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("env +(-S|--split-string)"));

fn check_shebang_parameters(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    match p.ast.get(t) {
        Token::T_Annotation(_, t) => check_shebang_parameters(p, *t),
        Token::T_Script(sb_id, _) => match p.ast.get(*sb_id) {
            Token::T_Literal(sb) => {
                let is_multi_word = words(sb).len() > 2 && !matches(sb, &RE);
                if is_multi_word {
                    vec![make_comment(
                        Severity::ErrorC,
                        *sb_id,
                        2096,
                        "On most OS, shebangs can only specify a single parameter.",
                    )]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        },
        _ => vec![],
    }
}
