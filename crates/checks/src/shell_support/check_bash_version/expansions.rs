// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Owned by L2: ${..} and brace expansions (SC9107 9108 9117 9125 9134 9135).
// Runs once per node, only when a bash target is set. Report with `super::too_new`.

use super::too_new;
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    get_associative_arrays, get_literal_string, is_special_variable_char, is_variable_char, oversimplify,
};

pub fn check(p: &Params<'_>, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_DollarBraced(_, word) => {
            let s: String = oversimplify(p.ast, *word).concat();
            braced(p, t, c, &s);
        }
        Token::T_BraceExpansion(items) => {
            if let [item] = items.as_slice() {
                if let Some(s) = get_literal_string(p.ast, *item) {
                    if is_zero_padded_range(&s) {
                        too_new(p, c, t, 9108);
                    }
                }
            }
        }
        _ => {}
    }
}

/// The inside of `${..}` split into its parts: a `#` or `!` prefix, the index text of
/// `name[index]` and what follows (the modifier, like `:0:-1` or `@Q`).
struct Braced<'a> {
    prefix: Option<char>,
    index: Option<&'a str>,
    modifier: &'a str,
}

fn split_braced(s: &str) -> Braced<'_> {
    let mut rest = s;
    let mut prefix = None;
    if let Some(ch) = rest.chars().next() {
        // `${#}` and `${!}` are the special parameters, not a prefix
        if (ch == '#' || ch == '!') && rest.len() > 1 {
            prefix = Some(ch);
            rest = &rest[1..];
        }
    }
    let name_len: usize = rest.chars().take_while(|&ch| is_variable_char(ch)).map(char::len_utf8).sum();
    if name_len > 0 {
        rest = &rest[name_len..];
    } else if let Some(ch) = rest.chars().next().filter(|&ch| is_special_variable_char(ch)) {
        rest = &rest[ch.len_utf8()..];
    }
    let mut index = None;
    if let Some(after) = rest.strip_prefix('[') {
        let mut depth = 1usize;
        for (i, ch) in after.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        index = Some(&after[..i]);
                        rest = &after[i + 1..];
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    Braced { prefix, index, modifier: rest }
}

fn braced(p: &Params<'_>, t: Id, c: &mut Comments, s: &str) {
    let b = split_braced(s);
    if let Some(ix) = b.index {
        // `${m[-1]}` on an associative array is a string key, valid in every version.
        // Only looked up when a negative index is seen, which keeps the common path cheap.
        if is_negative_index(ix) && !is_associative(p, s) {
            too_new(p, c, t, 9117);
        }
    }
    if b.prefix == Some('#') {
        return;
    }
    let m = b.modifier;
    if m.starts_with(['^', ',']) {
        too_new(p, c, t, 9107);
    } else if let Some(op) = m.strip_prefix('@') {
        let mut chars = op.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            match ch {
                'Q' | 'E' | 'P' | 'A' | 'a' => too_new(p, c, t, 9125),
                'U' | 'u' | 'L' | 'K' => too_new(p, c, t, 9134),
                'k' => too_new(p, c, t, 9135),
                _ => {}
            }
        }
    } else if let Some(sub) = m.strip_prefix(':') {
        // `${v:-x}`, `${v:=x}`, `${v:?x}` and `${v:+x}` are not substrings
        if !sub.starts_with(['-', '=', '?', '+']) {
            if let Some(len) = substring_length(sub) {
                if len.trim_start().starts_with('-') {
                    too_new(p, c, t, 9117);
                }
            }
        }
    }
}

/// Whether the array named at the start of `s` (after a `#`/`!` prefix) is declared
/// associative anywhere in the script.
fn is_associative(p: &Params<'_>, s: &str) -> bool {
    let s = s.strip_prefix(['#', '!']).unwrap_or(s);
    let name: String = s.chars().take_while(|&ch| is_variable_char(ch)).collect();
    // The tree is walked once per script, not once per negative index.
    let arrays = p.tree_cache.associative_arrays.get_or_init(|| get_associative_arrays(p.ast, p.root).into_iter().collect());
    !name.is_empty() && arrays.contains(&name)
}

/// A literal negative subscript: `-1`, ` -2 `. `$i-1` and `@` are not.
fn is_negative_index(ix: &str) -> bool {
    let rest = ix.trim().strip_prefix('-').map(str::trim_start);
    rest.is_some_and(|r| r.starts_with(|ch: char| ch.is_ascii_digit()))
}

/// The length part of `offset:length` (what follows the leading `:` of a substring),
/// skipping the `:` of `a?b:c` in the offset and any nested brackets.
fn substring_length(sub: &str) -> Option<&str> {
    let (mut depth, mut ternary) = (0i32, 0i32);
    for (i, ch) in sub.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '?' if depth == 0 => ternary += 1,
            ':' if depth == 0 => {
                if ternary > 0 {
                    ternary -= 1;
                } else {
                    return Some(&sub[i + 1..]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `01..10`, `-01..5`, `001..010..2`: a numeric range with a leading zero in a number of
/// two or more digits. Bash 4.0 added the padding.
fn is_zero_padded_range(s: &str) -> bool {
    let parts: Vec<&str> = s.split("..").collect();
    if !(2..=3).contains(&parts.len()) {
        return false;
    }
    let number = |x: &str| {
        let d = x.strip_prefix('-').unwrap_or(x);
        !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())
    };
    if !parts.iter().all(|x| number(x)) {
        return false;
    }
    parts[..2].iter().any(|x| {
        let d = x.strip_prefix('-').unwrap_or(x);
        d.len() > 1 && d.starts_with('0')
    })
}
