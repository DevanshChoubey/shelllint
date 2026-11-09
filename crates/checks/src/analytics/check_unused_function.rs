// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: not a port of an upstream check. shelllint's own optional check
// `unused-function` (SC9005, issue #612); it has no Haskell counterpart.

use std::collections::BTreeSet;

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_literal_string;
use shelllint_core::interface::TokenComment;

use crate::support::functions;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnusedFunction",
    registry: "optional-tree",
    order: 1000,
    optional: Some(StaticCheckDescription {
        name: "unused-function",
        id: &[9005],
        description: "Notify when a function is defined but never invoked in the script. \
Disabled for a file that runs a command whose name is only known at runtime. \
Libraries meant to be sourced by other scripts should leave this off, since their callers are unknowable",
        positive: "foo() { echo hi; }; echo done",
        negative: "foo() { echo hi; }; foo",
    }),
    kind: CheckKind::Tree(check_unused_function),
};

fn check_unused_function(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    let ast = params.ast;
    let defined = functions(ast, root);
    if defined.is_empty() {
        return vec![];
    }

    // Every name that appears as a word of the script, or inside a literal or
    // single quoted string (this covers `trap 'cleanup' EXIT`, `xargs fn`,
    // `export -f fn`, `eval 'fn'`). Being generous here only means fewer warnings.
    let mut mentioned: BTreeSet<String> = BTreeSet::new();
    let mut dynamic = false;
    ast.analyze(
        root,
        &mut |x| match ast.get(x) {
            Token::T_Literal(s) | Token::T_SingleQuoted(s) => {
                for w in s.split(is_separator).filter(|w| !w.is_empty()) {
                    mentioned.insert(w.to_string());
                }
            }
            Token::T_SimpleCommand(_, words) => {
                if let Some(&first) = words.first() {
                    match get_literal_string(ast, first) {
                        None => dynamic = true,
                        Some(name) => {
                            // `eval "$x"` and friends run a name chosen at runtime.
                            if matches!(name.as_str(), "eval" | "command" | "builtin")
                                && words[1..].iter().any(|&w| get_literal_string(ast, w).is_none())
                            {
                                dynamic = true;
                            }
                        }
                    }
                }
            }
            _ => {}
        },
        &mut |_| {},
    );
    if dynamic {
        return vec![];
    }

    let mut out: Vec<(Id, String)> = defined
        .into_iter()
        .filter(|(name, _)| !mentioned.contains(name))
        .map(|(name, id)| (id, name))
        .collect();
    out.sort_by_key(|(id, _)| id.0);
    let mut c = Comments::new();
    for (id, name) in out {
        c.style(id, 9005, format!("{name} is defined but never invoked in this script."));
    }
    c.into_vec()
}

fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '<' | '>' | '"' | '\'' | '`' | '$' | '{' | '}' | '=')
}
