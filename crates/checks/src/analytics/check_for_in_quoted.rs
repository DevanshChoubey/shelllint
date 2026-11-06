// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkForInQuoted`, lines 730-751 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    get_literal_string, get_trailing_unquoted_literal, get_unquoted_literal,
    may_become_multiple_args, will_split,
};

use crate::support::{fix_with, replace_end};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkForInQuoted",
    registry: "node",
    order: 1,
    optional: None,
    kind: CheckKind::Node(check_for_in_quoted),
};

fn would_have_been_glob(s: &str) -> bool {
    s.contains('*')
}

fn check_for_in_quoted(params: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = params.ast;
    let Token::T_ForIn(_, words, _) = ast.get(t) else { return };

    if let [w] = &words[..] {
        // Clause 1
        if let Token::T_NormalWord(parts) = ast.get(*w) {
            if let [word] = &parts[..] {
                let word = *word;
                if let Token::T_DoubleQuoted(list) = ast.get(word) {
                    if list.iter().any(|&x| will_split(ast, x)) && !may_become_multiple_args(ast, word)
                        || get_literal_string(ast, word).is_some_and(|s| would_have_been_glob(&s))
                    {
                        c.err(
                            word,
                            2066,
                            "Since you double quoted this, it will not word split, and the loop will only run once.",
                        );
                        return;
                    }
                }
                // Clause 2
                if let Token::T_SingleQuoted(_) = ast.get(word) {
                    c.warn(
                        word,
                        2041,
                        "This is a literal string. To run as a command, use $(..) instead of '..' . ",
                    );
                    return;
                }
            }
        }
        // Clause 3
        let single = *w;
        if get_unquoted_literal(ast, single).is_some_and(|s| s.contains(',')) {
            c.warn(single, 2042, "Use spaces, not commas, to separate loop elements.");
            return;
        } else if !(will_split(ast, single) || may_become_multiple_args(ast, single)) {
            c.warn(
                single,
                2043,
                "This loop will only ever run once. Bad quoting or missing glob/expansion?",
            );
            return;
        }
    }

    // Clause 4
    for &arg in words {
        let Some(suffix) = get_trailing_unquoted_literal(ast, arg) else { continue };
        let Some(string) = get_literal_string(ast, suffix) else { continue };
        if !string.ends_with(',') {
            continue;
        }
        c.warn_with_fix(
            arg,
            2258,
            "The trailing comma is part of the value, not a separator. Delete or quote it.",
            fix_with(vec![replace_end(suffix, params, 1, "")]),
        );
    }
}
