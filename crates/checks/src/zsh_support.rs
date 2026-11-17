// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only helpers for adapting the checks to zsh (no ShellCheck counterpart).
//
// zsh does not split or glob the result of an unquoted `$x` by default; it does
// so under `SH_WORD_SPLIT` / `GLOB_SUBST`, or where the script asks explicitly
// with `${=x}` (split) and `${~x}` (glob). Non-zsh shells answer the option
// queries with today's behaviour (`Params::zsh_option`), so a check that asks
// through these helpers is unchanged for them.

use std::collections::HashSet;

use shelllint_analyzer::analyzerlib::{Params, ZshOpt, make_comment};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{arguments, get_command_name, get_literal_string, oversimplify};
use shelllint_core::interface::{Severity, Shell, TokenComment};
use shelllint_core::zsh_data::{is_zsh_hook_array, is_zsh_implicit_function};

/// The leading `=` or `~` of `${=x}` / `${~x}` (zsh's explicit split and glob
/// flags), after an optional `(flags)` group. `None` for any other expansion.
pub fn explicit_flag(p: &Params<'_>, t: Id) -> Option<char> {
    // Only zsh gives these flags a meaning; other shells keep today's reading.
    if p.shell_type != Shell::Zsh {
        return None;
    }
    let Token::T_DollarBraced(_, l) = p.ast.get(t) else { return None };
    let s = oversimplify(p.ast, *l).concat();
    let rest = match s.strip_prefix('(') {
        Some(r) => &r[r.find(')')? + 1..],
        None => &s[..],
    };
    rest.strip_prefix('^').unwrap_or(rest).chars().next().filter(|c| matches!(c, '=' | '~'))
}

/// `${=x}`: split on `$IFS` even when `SH_WORD_SPLIT` is off.
pub fn explicit_split(p: &Params<'_>, t: Id) -> bool {
    explicit_flag(p, t) == Some('=')
}

/// `${~x}`: the result is a pattern even when `GLOB_SUBST` is off.
pub fn explicit_glob(p: &Params<'_>, t: Id) -> bool {
    explicit_flag(p, t) == Some('~')
}

/// Does an unquoted expansion `t` get split by the shell? `SH_WORD_SPLIT` on, or `${=x}`.
pub fn splits(p: &Params<'_>, t: Id) -> bool {
    p.zsh_option(t, ZshOpt::ShWordSplit) || explicit_split(p, t)
}

/// Does an unquoted expansion `t` get used as a glob pattern? `GLOB_SUBST` on, or `${~x}`.
pub fn globs(p: &Params<'_>, t: Id) -> bool {
    p.zsh_option(t, ZshOpt::GlobSubst) || explicit_glob(p, t)
}

/// Is `place`, a variable reference found by the data flow, really a literal
/// key in the subscript of a zsh special associative array (`$+commands[git]`,
/// `compstate[pattern_match]=x`, `${functions[name]}`)? Such a key is a string,
/// not an arithmetic expression, so the "variable" it names is never read.
pub fn is_zsh_assoc_key(p: &Params<'_>, place: Id) -> bool {
    use shelllint_core::astlib::{get_braced_reference, get_path};
    use shelllint_core::zsh_data::is_zsh_special_assoc;
    // A real `$name` (itself a `${..}`) inside the subscript is a read of `name`.
    if p.shell_type != Shell::Zsh || matches!(p.ast.get(place), Token::T_DollarBraced(..)) {
        return false;
    }
    for anc in get_path(p.ast, place).into_iter().skip(1) {
        match p.ast.get(anc) {
            Token::TA_Variable(name, _) => return is_zsh_special_assoc(name),
            Token::T_DollarBraced(_, l) => {
                return is_zsh_special_assoc(&get_braced_reference(&oversimplify(p.ast, *l).concat()));
            }
            Token::T_Assignment(_, name, _, _) => return is_zsh_special_assoc(name),
            Token::T_SimpleCommand(..) | Token::T_Function(..) => return false,
            _ => {}
        }
    }
    false
}

/// Names of functions that zsh itself calls, found in the script: hook and
/// `TRAPxxx` functions (by name), and anything registered with
/// `add-zsh-hook <hook> fn`, `zle -N [widget] fn` (also `-C`, `-F`),
/// `compdef fn cmd...`, `<hook>_functions+=(fn)` or `trap fn SIG`.
/// Empty unless the shell is zsh. `trap` is left to the check that already
/// knows it, but a name given to `trap` is included here too so SC2119/SC2120
/// see it.
pub fn zsh_registered_functions(p: &Params<'_>) -> HashSet<String> {
    let mut out = HashSet::new();
    if p.shell_type != Shell::Zsh {
        return out;
    }
    let ast = p.ast;
    ast.analyze(
        p.root,
        &mut |t| match ast.get(t) {
            Token::T_Function(_, _, name, _) if is_zsh_implicit_function(name) => {
                out.insert(name.clone());
            }
            Token::T_SimpleCommand(..) => {
                let Some(name) = get_command_name(ast, t) else { return };
                let args: Vec<(String, bool)> = arguments(ast, t)
                    .into_iter()
                    .filter_map(|a| get_literal_string(ast, a))
                    .map(|s| {
                        let is_flag = s.starts_with('-') && s.len() > 1;
                        (s, is_flag)
                    })
                    .collect();
                let positional = || args.iter().filter(|(_, f)| !f).map(|(s, _)| s.clone());
                let flags_have = |c: char| args.iter().any(|(s, f)| *f && s[1..].contains(c));
                match name.as_str() {
                    // add-zsh-hook [-dD] hook fn...: every operand after the hook
                    "add-zsh-hook" => out.extend(positional().skip(1)),
                    // zle -N [widget] fn, zle -C widget style fn, zle -F fd fn
                    "zle" if flags_have('N') || flags_have('C') || flags_have('F') => out.extend(positional()),
                    // compdef fn cmd... (the function is the first operand)
                    "compdef" => out.extend(positional().take(1)),
                    "trap" => out.extend(positional().take(1)),
                    _ => {}
                }
            }
            Token::T_Assignment(_, name, _, value) if is_zsh_hook_array(name) => {
                let words: Vec<Id> = match ast.get(*value) {
                    Token::T_Array(l) => l.clone(),
                    _ => vec![*value],
                };
                out.extend(words.into_iter().filter_map(|w| get_literal_string(ast, w)));
            }
            _ => {}
        },
        &mut |_| {},
    );
    out
}

/// Inside double quotes, does the zsh expansion `t` still produce several
/// words? That is `"${(@)a}"` and the splitting flags `s`, `f`, `z` and `0`
/// (`"${(s: :)x}"`, `"${(f)x}"`), and `"${=x}"`. Always false for other shells.
pub fn quoted_expansion_yields_words(p: &Params<'_>, t: Id) -> bool {
    if p.shell_type != Shell::Zsh {
        return false;
    }
    let Token::T_DollarBraced(_, l) = p.ast.get(t) else { return false };
    let s = oversimplify(p.ast, *l).concat();
    if s.starts_with('=') {
        return true;
    }
    let Some(flags) = s.strip_prefix('(').and_then(|r| r.find(')').map(|i| &r[..i])) else { return false };
    // Skip the delimited argument of `s`, `j`, `l` and `r` (the `: :` of `s: :`).
    fn skip_arg(chars: &mut std::str::Chars<'_>) {
        let Some(open) = chars.next() else { return };
        let close = match open {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            '<' => '>',
            x => x,
        };
        for x in chars.by_ref() {
            if x == close {
                break;
            }
        }
    }
    let mut chars = flags.chars();
    let mut found = false;
    while let Some(c) = chars.next() {
        match c {
            '@' | 'f' | 'z' | '0' => found = true,
            's' => {
                found = true;
                skip_arg(&mut chars);
            }
            'j' | 'l' | 'r' => skip_arg(&mut chars),
            _ => {}
        }
    }
    found
}

/// A word that has a double-quoted part with such an expansion
/// (see [`quoted_expansion_yields_words`]).
pub fn quoted_word_yields_words(p: &Params<'_>, w: Id) -> bool {
    let Token::T_NormalWord(parts) = p.ast.get(w) else { return false };
    parts.iter().any(|&x| match p.ast.get(x) {
        Token::T_DoubleQuoted(l) => l.iter().any(|&y| quoted_expansion_yields_words(p, y)),
        _ => false,
    })
}

/// The variable a zsh expansion with leading flags reads: `x` for `${(f)x}`,
/// `${=x}`, `${~x}`, `${^x}`, `${+x}`, `${(P)x}[1]` and `${(f)=x}`. The shared
/// `get_braced_reference` knows only the bash prefixes, so for these it finds no
/// variable. `None` when `t` is not such an expansion, or for other shells.
pub fn zsh_flagged_reference(p: &Params<'_>, t: Id) -> Option<String> {
    if p.shell_type != Shell::Zsh {
        return None;
    }
    let Token::T_DollarBraced(_, l) = p.ast.get(t) else { return None };
    let s = oversimplify(p.ast, *l).concat();
    let mut rest = &s[..];
    let mut stripped = false;
    loop {
        if let Some(r) = rest.strip_prefix('(') {
            rest = &r[r.find(')')? + 1..];
            stripped = true;
        } else if let Some(c) = rest.chars().next().filter(|c| matches!(c, '=' | '~' | '^' | '+')) {
            rest = &rest[c.len_utf8()..];
            stripped = true;
        } else {
            break;
        }
    }
    if !stripped {
        return None;
    }
    let name = shelllint_core::astlib::get_braced_reference(rest);
    if name.is_empty() { None } else { Some(name) }
}

/// SC1087 under zsh. The parser cannot see options, so it never reports
/// "Use braces when expanding arrays" for zsh; this reports it, with the same
/// message at the same position (the `$`), for each unbraced `$name[...]`
/// that appears where `KSH_ARRAYS` is on (there `$a[1]` is `${a}[1]`, not a
/// subscript). Other shells report it from the parser, so this is a no-op.
pub fn zsh_ksharrays_unbraced_subscripts(p: &Params<'_>) -> Vec<TokenComment> {
    if p.shell_type != Shell::Zsh {
        return Vec::new();
    }
    let mut found = Vec::new();
    p.ast.analyze(
        p.root,
        &mut |id| {
            // zsh reads `$a[1]` as `T_DollarBraced(true, a[1])` whose word
            // starts right after the `$`; `${a[1]}` has a `{` in between.
            if let Token::T_DollarBraced(true, word) = p.ast.get(id) {
                let (start, _) = p.ast.span(id);
                let (wstart, _) = p.ast.span(*word);
                if wstart.line == start.line && wstart.column == start.column + 1 {
                    found.push(id);
                }
            }
        },
        &mut |_| {},
    );
    found
        .into_iter()
        .filter(|id| p.zsh_option(*id, ZshOpt::KshArrays))
        .map(|id| {
            make_comment(
                Severity::ErrorC,
                id,
                1087,
                "Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).",
            )
        })
        .collect()
}
