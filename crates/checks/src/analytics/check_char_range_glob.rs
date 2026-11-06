// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCharRangeGlob`, lines 2878-2912 (e11f964)

use shellcheck_analyzer::analyzerlib::{
    Comments, Params, get_closest_command, is_command_match, is_dereferencing_binary_op,
};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{ConditionType, Id, Token};
use shellcheck_core::astlib::get_path;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCharRangeGlob",
    registry: "node",
    order: 45,
    optional: None,
    kind: CheckKind::Node(check_char_range_glob),
};

fn check_char_range_glob(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_Glob(str) = p.ast.get(t) else { return };
    let id = t;
    let chars: Vec<char> = str.chars().collect();

    let is_char_class = |s: &str| s.starts_with('[') && s.ends_with(']');
    if !is_char_class(str) {
        return;
    }

    // contents = dropNegation . drop 1 . take (length str - 1) $ str
    let taken: Vec<char> = chars[..chars.len().saturating_sub(1)].to_vec();
    let dropped: &[char] = if taken.is_empty() { &taken[..] } else { &taken[1..] };
    let contents: Vec<char> = match dropped {
        ['!', rest @ ..] | ['^', rest @ ..] => rest.to_vec(),
        x => x.to_vec(),
    };

    let is_ignored_command = match get_closest_command(p.ast, t) {
        Some(cmd) => is_command_match(p.ast, cmd, &|s| s == "tr" || s == "read"),
        None => false,
    };
    if is_ignored_command {
        return;
    }

    // Check if this is a dereferencing context like [[ -v array[operandhere] ]]
    let is_dereferenced = {
        let mut res = false;
        for x in get_path(p.ast, t) {
            match p.ast.get(x) {
                Token::TC_Binary(ConditionType::DoubleBracket, s, _, _) => {
                    res = is_dereferencing_binary_op(s);
                    break;
                }
                Token::TC_Unary(_, s, _) => {
                    res = s == "-v";
                    break;
                }
                Token::T_SimpleCommand(..) => {
                    res = false;
                    break;
                }
                _ => {}
            }
        }
        res
    };
    if is_dereferenced {
        return;
    }

    let n = contents.len();
    if n > 0 && contents[0] == ':' && contents[n - 1] == ':' && contents != [':'] {
        c.warn(id, 2101, "Named class needs outer [], e.g. [[:digit:]].");
    } else {
        // hasDupes = any ((>1) . length) . group . sort . filter (/= '-') $ contents
        let mut sorted: Vec<char> = contents.iter().copied().filter(|&ch| ch != '-').collect();
        sorted.sort();
        let has_dupes = sorted.windows(2).any(|w| w[0] == w[1]);
        if !contents.contains(&'[') && has_dupes {
            c.info(id, 2102, "Ranges can only match single chars (mentioned due to duplicates).");
        }
    }
}
