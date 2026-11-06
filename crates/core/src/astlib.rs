// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/ASTLib.hs (e11f964)
//
// Every function takes the `&Ast` arena first and refers to tokens by `Id`.
// Not ported: the TemplateHaskell `runTests = $quickCheckAll` footer (the
// `prop_` definitions live in `crates/analyzer/tests/astlib.rs`).
//
// DIVERGENCE: `getLiteralStringExt :: Monad m => ...` is only ever used at
// `Maybe` and `Identity`, so it is specialised to `Option`; the `Identity`
// user (`getLiteralStringDef`) passes a callback that always succeeds.
#![allow(non_camel_case_types)]

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use crate::ast::{Annotation, Ast, Id, Token};
use crate::prelude::please_report;
use crate::regex::{match_all_strings, match_regex, matches, mk_regex};

/// `(String, (Token, Token))` as returned by the getOpts family.
pub type Opt = (String, (Id, Id));

pub fn arguments(ast: &Ast, t: Id) -> Vec<Id> {
    match ast.get(t) {
        Token::T_SimpleCommand(_, words) if !words.is_empty() => words[1..].to_vec(),
        // Haskell: non-exhaustive pattern match
        _ => panic!("{}", please_report("arguments on non-command")),
    }
}

/// Is this a type of loop?
pub fn is_loop(ast: &Ast, t: Id) -> bool {
    matches!(
        ast.get(t),
        Token::T_WhileExpression(..)
            | Token::T_UntilExpression(..)
            | Token::T_ForIn(..)
            | Token::T_ForArithmetic(..)
            | Token::T_SelectIn(..)
    )
}

/// Will this split into multiple words when used as an argument?
pub fn will_split(ast: &Ast, x: Id) -> bool {
    match ast.get(x) {
        Token::T_DollarBraced(..) => true,
        Token::T_DollarExpansion(..) => true,
        Token::T_Backticked(..) => true,
        Token::T_BraceExpansion(..) => true,
        Token::T_Glob(..) => true,
        Token::T_Extglob(..) => true,
        Token::T_DoubleQuoted(l) => l.iter().any(|&p| will_become_multiple_args(ast, p)),
        Token::T_NormalWord(l) => l.iter().any(|&p| will_split(ast, p)),
        _ => false,
    }
}

pub fn is_glob(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Extglob(..) => true,
        Token::T_Glob(..) => true,
        Token::T_NormalWord(l) => {
            l.iter().any(|&p| is_glob(ast, p)) || has_split_range(ast, l)
        }
        _ => false,
    }
}

// foo[x${var}y] gets parsed as foo,[,x,$var,y],
// so check if there's such an interval
fn has_split_range(ast: &Ast, l: &[Id]) -> bool {
    let start = l.iter().position(|&t| is_half_open_range(ast, t));
    match start {
        None => false,
        Some(i) => l[i..].iter().any(|&t| is_closing_range(ast, t)),
    }
}

fn is_half_open_range(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_Literal(s) if s == "[")
}

fn is_closing_range(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_Literal(s) if s.contains(']'))
}

/// Is this shell word a constant?
pub fn is_constant(ast: &Ast, token: Id) -> bool {
    match ast.get(token) {
        // This ignores some cases like ~"foo":
        Token::T_NormalWord(l)
            if matches!(l.first().map(|&f| ast.get(f)), Some(Token::T_Literal(s)) if s.starts_with('~')) =>
        {
            false
        }
        Token::T_NormalWord(l) => l.iter().all(|&p| is_constant(ast, p)),
        Token::T_DoubleQuoted(l) => l.iter().all(|&p| is_constant(ast, p)),
        Token::T_SingleQuoted(_) => true,
        Token::T_Literal(_) => true,
        _ => false,
    }
}

/// Is this an empty literal?
pub fn is_empty(ast: &Ast, token: Id) -> bool {
    match ast.get(token) {
        Token::T_NormalWord(l) => l.iter().all(|&p| is_empty(ast, p)),
        Token::T_DoubleQuoted(l) => l.iter().all(|&p| is_empty(ast, p)),
        Token::T_SingleQuoted(s) => s.is_empty(),
        Token::T_Literal(s) => s.is_empty(),
        _ => false,
    }
}

/// Quick&lazy oversimplification of commands, throwing away details
/// and returning a list like  ["find", ".", "-name", "${VAR}*" ].
pub fn oversimplify(ast: &Ast, token: Id) -> Vec<String> {
    let many = |l: &[Id]| -> Vec<String> { l.iter().flat_map(|&p| oversimplify(ast, p)).collect() };
    match ast.get(token) {
        Token::T_NormalWord(l) => vec![many(l).concat()],
        Token::T_DoubleQuoted(l) => vec![many(l).concat()],
        Token::T_SingleQuoted(s) => vec![s.clone()],
        Token::T_DollarBraced(..) => vec!["${VAR}".to_string()],
        Token::T_DollarArithmetic(_) => vec!["${VAR}".to_string()],
        Token::T_DollarExpansion(_) => vec!["${VAR}".to_string()],
        Token::T_Backticked(_) => vec!["${VAR}".to_string()],
        Token::T_Glob(s) => vec![s.clone()],
        Token::T_Pipeline(_, cmds) if cmds.len() == 1 => oversimplify(ast, cmds[0]),
        Token::T_Literal(x) => vec![x.clone()],
        Token::T_ParamSubSpecialChar(x) => vec![x.clone()],
        Token::T_SimpleCommand(_vars, words) => many(words),
        Token::T_Redirecting(_, foo) => oversimplify(ast, *foo),
        Token::T_DollarSingleQuoted(s) => vec![s.clone()],
        Token::T_Annotation(_, s) => oversimplify(ast, *s),
        // Workaround for let "foo = bar" parsing
        Token::TA_Sequence(l) if l.len() == 1 => match ast.get(l[0]) {
            Token::TA_Expansion(v) => many(v),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// Turn a SimpleCommand foo -avz --bar=baz into args "a", "v", "z", "bar",
/// each in a tuple of (token, stringFlag). Non-flag arguments are added with
/// stringFlag == "".
pub fn get_flags_until(ast: &Ast, stop_condition: &dyn Fn(&str) -> bool, t: Id) -> Vec<(Id, String)> {
    match ast.get(t) {
        Token::T_SimpleCommand(_, words) if !words.is_empty() => {
            get_flags_until_words(ast, stop_condition, &words[1..])
        }
        _ => panic!("{}", please_report("getFlags on non-command")),
    }
}

/// `getFlagsUntil` on the arguments of a command (its words after the
/// command name), for callers that hold a word list rather than a
/// `T_SimpleCommand` token (e.g. AnalyzerLib's synthetic `builtin` command).
pub fn get_flags_until_words(
    ast: &Ast,
    stop_condition: &dyn Fn(&str) -> bool,
    args: &[Id],
) -> Vec<(Id, String)> {
    let token_and_text: Vec<(Id, String)> =
        args.iter().map(|&x| (x, oversimplify(ast, x).concat())).collect();
    let split = token_and_text
        .iter()
        .position(|(_, s)| stop_condition(s))
        .unwrap_or(token_and_text.len());
    let (flag_args, rest) = token_and_text.split_at(split);
    let mut out: Vec<(Id, String)> = flag_args.iter().flat_map(|(x, s)| flag(*x, s)).collect();
    out.extend(rest.iter().map(|(t, _)| (*t, String::new())));
    out
}

fn flag(x: Id, s: &str) -> Vec<(Id, String)> {
    if let Some(arg) = s.strip_prefix("--") {
        vec![(x, arg.chars().take_while(|&c| c != '=').collect())]
    } else if let Some(args) = s.strip_prefix('-') {
        args.chars().map(|v| (x, v.to_string())).collect()
    } else {
        vec![(x, String::new())]
    }
}

/// Get all flags in a GNU way, up until --
pub fn get_all_flags(ast: &Ast, t: Id) -> Vec<(Id, String)> {
    get_flags_until(ast, &|s| s == "--", t)
}

/// Get all flags in a BSD way, up until first non-flag argument or --
pub fn get_leading_flags(ast: &Ast, t: Id) -> Vec<(Id, String)> {
    get_flags_until(ast, &|x| x == "--" || !x.starts_with('-'), t)
}

/// Check if a command has a flag.
pub fn has_flag(ast: &Ast, cmd: Id, str: &str) -> bool {
    get_all_flags(ast, cmd).iter().any(|(_, s)| s == str)
}

/// Is this token a word that starts with a dash?
pub fn is_flag(ast: &Ast, token: Id) -> bool {
    match get_word_parts(ast, token).first().map(|&p| ast.get(p)) {
        Some(Token::T_Literal(s)) => s.starts_with('-'),
        _ => false,
    }
}

/// Is this token a flag where the - is unquoted?
pub fn is_unquoted_flag(ast: &Ast, token: Id) -> bool {
    matches!(get_leading_unquoted_string(ast, token), Some(s) if s.starts_with('-'))
}

/// getGnuOpts "erd:u:" will parse a list of arguments tokens like `read`
///     -re -d : -u 3 bar
/// into
///     Just [("r", (-re, -re)), ("e", (-re, -re)), ("d", (-d,:)), ("u", (-u,3)), ("", (bar,bar))]
///
/// Each string flag maps to a tuple of (flag, argument), where argument=flag if it
/// doesn't take a specific one.
///
/// Any unrecognized flag will result in Nothing. The exception is if arbitraryLongOpts
/// is set, in which case --anything will map to "anything".
pub fn get_gnu_opts(ast: &Ast, str: &str, args: &[Id]) -> Option<Vec<Opt>> {
    get_opts(ast, (true, false), str, &[], args)
}

/// As above, except the first non-arg string will treat the rest as arguments
pub fn get_bsd_opts(ast: &Ast, str: &str, args: &[Id]) -> Option<Vec<Opt>> {
    get_opts(ast, (false, false), str, &[], args)
}

/// Tests for this are in Commands.hs where it's more frequently used
///
/// `(gnu, arbitrary_long_opts)`: behavioral config; `string`: a getopts style
/// string; `longopts`: long options and whether they take arguments; `args`:
/// arguments (excluding command).
// DIVERGENCE: Haskell's `Maybe` monad (`fail`, pattern-match failure in `do`)
// becomes `Option` with `?`; the recursion is kept.
pub fn get_opts(
    ast: &Ast,
    (gnu, arbitrary_long_opts): (bool, bool),
    string: &str,
    longopts: &[(String, bool)],
    args: &[Id],
) -> Option<Vec<Opt>> {
    fn flag_list(chars: &[char], longopts: &[(String, bool)]) -> Vec<(String, bool)> {
        match chars {
            [c, ':', rest @ ..] => {
                let mut v = vec![(c.to_string(), true)];
                v.extend(flag_list(rest, longopts));
                v
            }
            [c, rest @ ..] => {
                let mut v = vec![(c.to_string(), false)];
                v.extend(flag_list(rest, longopts));
                v
            }
            [] => longopts.to_vec(),
        }
    }
    let chars: Vec<char> = string.chars().collect();
    // Map.fromList: later entries override earlier ones.
    let mut flag_map: HashMap<String, bool> = HashMap::new();
    flag_map.insert(String::new(), false);
    for (k, v) in flag_list(&chars, longopts) {
        flag_map.insert(k, v);
    }
    let ctx = OptsCtx { ast, gnu, arbitrary_long_opts, flag_map };
    ctx.process(args)
}

struct OptsCtx<'a> {
    ast: &'a Ast,
    gnu: bool,
    arbitrary_long_opts: bool,
    flag_map: HashMap<String, bool>,
}

impl OptsCtx<'_> {
    fn process(&self, args: &[Id]) -> Option<Vec<Opt>> {
        let Some((&token, rest)) = args.split_first() else {
            return Some(vec![]);
        };
        let s = get_literal_string_def(self.ast, "\0", token);
        if s == "--" {
            Some(list_to_args(rest))
        } else if let Some(word) = s.strip_prefix("--") {
            let eq = word.find('=').unwrap_or(word.len());
            let (name, arg) = word.split_at(eq);
            let needs_arg = if self.arbitrary_long_opts {
                *self.flag_map.get(name).unwrap_or(&false)
            } else {
                *self.flag_map.get(name)?
            };
            if needs_arg && arg.is_empty() {
                match rest.split_first() {
                    Some((&arg, rest2)) => {
                        let more = self.process(rest2)?;
                        let mut out = vec![(name.to_string(), (token, arg))];
                        out.extend(more);
                        Some(out)
                    }
                    None => None, // fail "Missing arg"
                }
            } else {
                let more = self.process(rest)?;
                // Consider splitting up token to get arg
                let mut out = vec![(name.to_string(), (token, token))];
                out.extend(more);
                Some(out)
            }
        } else if let Some(opts) = s.strip_prefix('-') {
            let opts: Vec<char> = opts.chars().collect();
            self.short_to_opts(&opts, token, rest)
        } else if self.gnu {
            let more = self.process(rest)?;
            let mut out = vec![(String::new(), (token, token))];
            out.extend(more);
            Some(out)
        } else {
            let mut all = vec![token];
            all.extend_from_slice(rest);
            Some(list_to_args(&all))
        }
    }

    fn short_to_opts(&self, opts: &[char], token: Id, args: &[Id]) -> Option<Vec<Opt>> {
        match opts.split_first() {
            Some((&c, rest)) => {
                let needs_arg = *self.flag_map.get(&c.to_string())?;
                if needs_arg && rest.is_empty() {
                    let (&next, rest_args) = args.split_first()?;
                    let more = self.process(rest_args)?;
                    let mut out = vec![(c.to_string(), (token, next))];
                    out.extend(more);
                    Some(out)
                } else if needs_arg {
                    let more = self.process(args)?;
                    let mut out = vec![(c.to_string(), (token, token))];
                    out.extend(more);
                    Some(out)
                } else {
                    let more = self.short_to_opts(rest, token, args)?;
                    let mut out = vec![(c.to_string(), (token, token))];
                    out.extend(more);
                    Some(out)
                }
            }
            None => self.process(args),
        }
    }
}

fn list_to_args(l: &[Id]) -> Vec<Opt> {
    l.iter().map(|&x| (String::new(), (x, x))).collect()
}

/// Generic getOpts that doesn't rely on a format string, but may also be inaccurate.
/// This provides a best guess interpretation instead of failing when new options are added.
///
///    "--" is treated as end of arguments
///    "--anything[=foo]" is treated as a long option without argument
///    "-any" is treated as -a -n -y, with the next arg as an option to -y unless it starts with -
///    anything else is an argument
pub fn get_generic_opts(ast: &Ast, tokens: &[Id]) -> Vec<Opt> {
    let Some((&token, rest)) = tokens.split_first() else {
        return vec![];
    };
    let s = get_literal_string_def(ast, "\0", token);
    if s == "--" {
        list_to_args(rest)
    } else if let Some(word) = s.strip_prefix("--") {
        let name: String = word.chars().take_while(|&c| c != '\0' && c != '=').collect();
        let mut out = vec![(name, (token, token))];
        out.extend(get_generic_opts(ast, rest));
        out
    } else if let Some(opt_string) = s.strip_prefix('-') {
        let opts: Vec<char> = opt_string.chars().take_while(|&c| c != '\0').collect();
        let each = |cs: &[char]| -> Vec<Opt> {
            cs.iter().map(|c| (c.to_string(), (token, token))).collect()
        };
        match rest.split_first() {
            Some((&next, _)) if get_literal_string_def(ast, "\0", next).starts_with('-') => {
                let mut out = each(&opts);
                out.extend(get_generic_opts(ast, rest));
                out
            }
            Some((&next, remainder)) => match opts.split_last() {
                Some((&last, initial)) => {
                    let mut out = each(initial);
                    out.push((last.to_string(), (token, next)));
                    out.extend(get_generic_opts(ast, remainder));
                    out
                }
                None => get_generic_opts(ast, remainder),
            },
            None => each(&opts),
        }
    } else {
        let mut out = vec![(String::new(), (token, token))];
        out.extend(get_generic_opts(ast, rest));
        out
    }
}

/// Is this an expansion of multiple items of an array?
pub fn is_array_expansion(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_DollarBraced(_, l) => {
            let string = oversimplify(ast, *l).concat();
            string.starts_with('@') || (!string.starts_with('#') && string.contains("[@]"))
        }
        _ => false,
    }
}

/// Is it possible that this arg becomes multiple args?
pub fn may_become_multiple_args(ast: &Ast, t: Id) -> bool {
    fn f(ast: &Ast, quoted: bool, t: Id) -> bool {
        match ast.get(t) {
            Token::T_DollarBraced(_, l) => {
                let string = oversimplify(ast, *l).concat();
                !quoted || string.starts_with('!')
            }
            Token::T_DoubleQuoted(parts) => parts.iter().any(|&p| f(ast, true, p)),
            Token::T_NormalWord(parts) => parts.iter().any(|&p| f(ast, quoted, p)),
            _ => false,
        }
    }
    will_become_multiple_args(ast, t) || f(ast, false, t)
}

/// Is it certain that this word will becomes multiple words?
pub fn will_become_multiple_args(ast: &Ast, t: Id) -> bool {
    fn f(ast: &Ast, t: Id) -> bool {
        match ast.get(t) {
            Token::T_Extglob(..) | Token::T_Glob(..) | Token::T_BraceExpansion(..) => true,
            Token::T_NormalWord(parts) => parts.iter().any(|&p| f(ast, p)),
            _ => false,
        }
    }
    will_concat_in_assignment(ast, t) || f(ast, t)
}

/// This does token cause implicit concatenation in assignments?
pub fn will_concat_in_assignment(ast: &Ast, token: Id) -> bool {
    match ast.get(token) {
        Token::T_DollarBraced(..) => is_array_expansion(ast, token),
        Token::T_DoubleQuoted(parts) => parts.iter().any(|&p| will_concat_in_assignment(ast, p)),
        Token::T_NormalWord(parts) => parts.iter().any(|&p| will_concat_in_assignment(ast, p)),
        _ => false,
    }
}

/// Maybe get the literal string corresponding to this token
pub fn get_literal_string(ast: &Ast, t: Id) -> Option<String> {
    get_literal_string_ext(ast, &|_| None, t)
}

/// Definitely get a literal string, with a given default for all non-literals
pub fn get_literal_string_def(ast: &Ast, x: &str, t: Id) -> String {
    get_literal_string_ext(ast, &|_| Some(x.to_string()), t)
        .expect("callback always succeeds")
}

/// Definitely get a literal string, skipping over all non-literals
pub fn only_literal_string(ast: &Ast, t: Id) -> String {
    get_literal_string_def(ast, "", t)
}

/// Maybe get a literal string, but only if it's an unquoted argument.
pub fn get_unquoted_literal(ast: &Ast, t: Id) -> Option<String> {
    match ast.get(t) {
        Token::T_NormalWord(list) => {
            let mut out = String::new();
            for &p in list {
                match ast.get(p) {
                    Token::T_Literal(s) => out.push_str(s),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

pub fn is_quotes(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_DoubleQuoted(..) | Token::T_SingleQuoted(..))
}

/// Get the last unquoted T_Literal in a word like "${var}foo"THIS
/// or nothing if the word does not end in an unquoted literal.
pub fn get_trailing_unquoted_literal(ast: &Ast, t: Id) -> Option<Id> {
    match ast.get(t) {
        Token::T_NormalWord(list) => {
            let &last = list.last()?;
            match ast.get(last) {
                Token::T_Literal(..) => Some(last),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Get the leading, unquoted, literal string of a token (if any).
pub fn get_leading_unquoted_string(ast: &Ast, t: Id) -> Option<String> {
    match ast.get(t) {
        Token::T_NormalWord(l) => match l.split_first() {
            Some((&first, rest)) => match ast.get(first) {
                Token::T_Literal(s) => {
                    let mut out = s.clone();
                    for &p in rest {
                        match ast.get(p) {
                            Token::T_Literal(s) => out.push_str(s),
                            _ => break,
                        }
                    }
                    Some(out)
                }
                _ => None,
            },
            None => None,
        },
        _ => None,
    }
}

/// Maybe get the literal string of this token and any globs in it.
pub fn get_glob_or_literal_string(ast: &Ast, t: Id) -> Option<String> {
    get_literal_string_ext(
        ast,
        &|x| match ast.get(x) {
            Token::T_Glob(str) => Some(str.clone()),
            _ => None,
        },
        t,
    )
}

/// Maybe get the literal value of a token, using a custom function
/// to map unrecognized Tokens into strings.
pub fn get_literal_string_ext(ast: &Ast, more: &dyn Fn(Id) -> Option<String>, t: Id) -> Option<String> {
    let all_in_list = |l: &[Id]| -> Option<String> {
        let mut out = String::new();
        for &p in l {
            out.push_str(&get_literal_string_ext(ast, more, p)?);
        }
        Some(out)
    };
    match ast.get(t) {
        Token::T_DoubleQuoted(l) => all_in_list(l),
        Token::T_DollarDoubleQuoted(l) => all_in_list(l),
        Token::T_NormalWord(l) => all_in_list(l),
        Token::TA_Expansion(l) => all_in_list(l),
        Token::T_SingleQuoted(s) => Some(s.clone()),
        Token::T_Literal(s) => Some(s.clone()),
        Token::T_ParamSubSpecialChar(s) => Some(s.clone()),
        Token::T_DollarSingleQuoted(s) => Some(decode_escapes(s)),
        _ => more(t),
    }
}

// Haskell's readHex/readOct on a prefix: the value and the number of digits
// consumed, or None if there is not a single digit (readHex returns []).
fn read_radix(s: &[char], radix: u32) -> Option<(u64, usize)> {
    let mut n: u64 = 0;
    let mut k = 0;
    for &c in s {
        match c.to_digit(radix) {
            Some(d) if c.is_ascii() => {
                n = n.wrapping_mul(radix as u64).wrapping_add(d as u64);
                k += 1;
            }
            _ => break,
        }
    }
    if k == 0 { None } else { Some((n, k)) }
}

// DIVERGENCE: Haskell's `chr` accepts surrogates and crashes above U+10FFFF;
// Rust's `char` cannot hold surrogates, so any invalid code point becomes
// U+FFFD instead.
fn chr(n: u64) -> char {
    u32::try_from(n).ok().and_then(char::from_u32).unwrap_or('\u{FFFD}')
}

/// Bash style $'..' decoding
fn decode_escapes(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let len = cs.len();
    let mut out = String::new();
    let mut i = 0;
    while i < len {
        if cs[i] == '\\' && i + 1 < len {
            let c = cs[i + 1];
            let rest = i + 2; // index of `cs` in the Haskell clause
            let simple = match c {
                'a' => Some('\x07'),
                'b' => Some('\x08'),
                'e' | 'E' => Some('\x1B'),
                'f' => Some('\x0C'),
                'n' => Some('\n'),
                'r' => Some('\r'),
                't' => Some('\t'),
                'v' => Some('\x0B'),
                '\\' => Some('\\'),
                '\'' => Some('\''),
                '"' => Some('"'),
                '?' => Some('?'),
                _ => None,
            };
            if let Some(ch) = simple {
                out.push(ch);
                i = rest;
                continue;
            }
            match c {
                'x' | 'u' | 'U' => {
                    let n = match c {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let end = (rest + n).min(len);
                    let taken = &cs[rest..end];
                    match read_radix(taken, 16) {
                        Some((v, k)) => {
                            out.push(chr(v));
                            out.extend(&taken[k..]);
                            i = end;
                        }
                        None => {
                            // sic: ShellCheck emits \x for \u and \U as well
                            out.push('\\');
                            out.push('x');
                            i = rest;
                        }
                    }
                }
                _ => {
                    let end = (rest + 2).min(len);
                    let mut taken = vec![c];
                    taken.extend_from_slice(&cs[rest..end]);
                    match read_radix(&taken, 8) {
                        Some((v, k)) => {
                            out.push(chr(v % 256));
                            out.extend(&taken[k..]);
                            i = end;
                        }
                        None => {
                            out.push('\\');
                            out.push(c);
                            i = rest;
                        }
                    }
                }
            }
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

/// Is this token a string literal?
pub fn is_literal(ast: &Ast, t: Id) -> bool {
    get_literal_string(ast, t).is_some()
}

/// Is this token a string literal number?
pub fn is_literal_number(ast: &Ast, t: Id) -> bool {
    match get_literal_string(ast, t) {
        Some(s) => s.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// `e4m = escapeForMessage`
pub fn e4m(str: &str) -> String {
    escape_for_message(str)
}

/// Escape user data for messages.
/// Messages generally avoid repeating user data, but sometimes it's helpful.
// `shouldEscape c = not (isPrint c) || (not (isAscii c) && not (isLetter c))`.
// For ASCII, `isPrint` is ' '..='~'; a non-ASCII char is kept iff it is a
// letter (letters are always printable).
pub fn escape_for_message(str: &str) -> String {
    fn pad0(n: usize, s: String) -> String {
        let l = s.chars().count();
        if l < n { "0".repeat(n - l) + &s } else { s }
    }
    fn to_hex(c: char) -> String {
        format!("{:X}", c as u32)
    }
    fn should_escape(c: char) -> bool {
        if c.is_ascii() {
            !(' '..='~').contains(&c)
        } else {
            !crate::unicode::is_letter(c)
        }
    }
    let mut out = String::new();
    for c in str.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x1B' => out.push_str("\\e"),
            c if should_escape(c) => {
                if (c as u32) < 256 {
                    out.push_str("\\x");
                    out.push_str(&pad0(2, to_hex(c)));
                } else {
                    out.push_str("\\U");
                    out.push_str(&pad0(4, to_hex(c)));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Turn a NormalWord like foo="bar $baz" into a series of constituent elements like [foo=,bar ,$baz]
// DIVERGENCE: Haskell returns the (possibly synthetic) tokens; every element
// returned here is an existing node, so ids are returned.
pub fn get_word_parts(ast: &Ast, t: Id) -> Vec<Id> {
    match ast.get(t) {
        Token::T_NormalWord(l) => l.iter().flat_map(|&p| get_word_parts(ast, p)).collect(),
        Token::T_DoubleQuoted(l) => l.clone(),
        // TA_Expansion is basically T_NormalWord for arithmetic expressions
        Token::TA_Expansion(l) => l.iter().flat_map(|&p| get_word_parts(ast, p)).collect(),
        _ => vec![t],
    }
}

/// Return a list of NormalWords that would result from brace expansion
// DIVERGENCE: the Haskell builds new `T_NormalWord id items` tokens that reuse
// the original `id`. An arena cannot alias ids, so each result is a freshly
// allocated node (with the original's span) and `ast` is taken mutably.
// The Haskell is a lazy list-monad product cut by `take 1000`; here the
// product is built eagerly but capped at 1000 at every step (first part varies
// slowest, as in the list monad), which yields the same first 1000 results.
pub fn brace_expand(ast: &mut Ast, id: Id) -> Vec<Id> {
    let list = match ast.get(id) {
        Token::T_NormalWord(l) => l.clone(),
        _ => panic!("{}", please_report("braceExpand on non-NormalWord")),
    };
    let (start, end) = ast.span(id);
    let mut options: Vec<Vec<Id>> = Vec::new();
    for p in list {
        match ast.get(p).clone() {
            Token::T_BraceExpansion(items) => {
                let mut o = Vec::new();
                for item in items {
                    o.extend(brace_expand(ast, item));
                    if o.len() >= 1000 {
                        break;
                    }
                }
                o.truncate(1000);
                options.push(o);
            }
            _ => options.push(vec![p]),
        }
    }
    let mut combos: Vec<Vec<Id>> = vec![vec![]];
    for o in options {
        let mut next = Vec::new();
        'outer: for c in &combos {
            for &x in &o {
                let mut n = c.clone();
                n.push(x);
                next.push(n);
                if next.len() >= 1000 {
                    break 'outer;
                }
            }
        }
        combos = next;
    }
    combos
        .into_iter()
        .take(1000)
        .map(|items| {
            let n = ast.alloc(start, end);
            ast.set(n, Token::T_NormalWord(items));
            n
        })
        .collect()
}

/// Maybe get a SimpleCommand from immediate wrappers like T_Redirections
pub fn get_command(ast: &Ast, t: Id) -> Option<Id> {
    match ast.get(t) {
        Token::T_Redirecting(_, w) => get_command(ast, *w),
        Token::T_SimpleCommand(_, words) if !words.is_empty() => Some(t),
        Token::T_Annotation(_, t) => get_command(ast, *t),
        _ => None,
    }
}

/// Maybe get the command name string of a token representing a command
pub fn get_command_name(ast: &Ast, t: Id) -> Option<String> {
    get_command_name_and_token(ast, false, t).0
}

/// Maybe get the name+arguments of a command.
pub fn get_command_argv(ast: &Ast, t: Id) -> Option<Vec<Id>> {
    let cmd = get_command(ast, t)?;
    match ast.get(cmd) {
        Token::T_SimpleCommand(_, args) if !args.is_empty() => Some(args.clone()),
        _ => None,
    }
}

/// Get the command name token from a command, i.e.
/// the token representing 'ls' in 'ls -la 2> foo'.
/// If it can't be determined, return the original token.
pub fn get_command_token_or_this(ast: &Ast, t: Id) -> Id {
    get_command_name_and_token(ast, false, t).1
}

/// Given a command, get the string and token that represents the command name.
/// If direct, return the actual command (e.g. exec in 'exec ls')
/// If not, return the logical command (e.g. 'ls' in 'exec ls')
pub fn get_command_name_and_token(ast: &Ast, direct: bool, t: Id) -> (Option<String>, Id) {
    fn effective(ast: &Ast, str: &str, args: &[Id]) -> Option<Id> {
        let first_arg = || {
            let arg = *args.first()?;
            if is_flag(ast, arg) { None } else { Some(arg) }
        };
        match str {
            "busybox" => first_arg(),
            "builtin" => first_arg(),
            "command" => first_arg(),
            "run" => first_arg(), // Used by bats
            "exec" => {
                let opts = get_bsd_opts(ast, "cla:", args)?;
                let (_, (t, _)) = opts.into_iter().find(|(name, _)| name.is_empty())?;
                Some(t)
            }
            _ => None,
        }
    }
    let go = || -> Option<(Option<String>, Id)> {
        let cmd = get_command(ast, t)?;
        let Token::T_SimpleCommand(_, words) = ast.get(cmd) else { return None };
        let (&w, rest) = words.split_first()?;
        let s = get_literal_string(ast, w)?;
        let inner = || -> Option<(Option<String>, Id)> {
            if direct {
                return None;
            }
            let actual = effective(ast, &s, rest)?;
            Some((get_literal_string(ast, actual), actual))
        };
        Some(inner().unwrap_or((Some(s.clone()), w)))
    };
    go().unwrap_or((None, t))
}

/// If a command substitution is a single command, get its name.
///  $(date +%s) = Just "date"
pub fn get_command_name_from_expansion(ast: &Ast, t: Id) -> Option<String> {
    let extract = |c: Id| match ast.get(c) {
        Token::T_Pipeline(_, cmds) if cmds.len() == 1 => get_command_name(ast, cmds[0]),
        _ => None,
    };
    match ast.get(t) {
        Token::T_DollarExpansion(l) if l.len() == 1 => extract(l[0]),
        Token::T_Backticked(l) if l.len() == 1 => extract(l[0]),
        Token::T_DollarBraceCommandExpansion(_, l) if l.len() == 1 => extract(l[0]),
        _ => None,
    }
}

/// Get the basename of a token representing a command
pub fn get_command_basename(ast: &Ast, t: Id) -> Option<String> {
    get_command_name(ast, t).map(|s| basename(&s))
}

pub fn basename(s: &str) -> String {
    s.rsplit('/').next().unwrap_or("").to_string()
}

pub fn is_assignment(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Redirecting(_, w) => is_assignment(ast, *w),
        Token::T_SimpleCommand(vars, words) => !vars.is_empty() && words.is_empty(),
        Token::T_Assignment(..) => true,
        Token::T_Annotation(_, w) => is_assignment(ast, *w),
        _ => false,
    }
}

pub fn is_only_redirection(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Pipeline(_, cmds) if cmds.len() == 1 => is_only_redirection(ast, cmds[0]),
        Token::T_Annotation(_, w) => is_only_redirection(ast, *w),
        Token::T_Redirecting(redirs, c) if !redirs.is_empty() => is_only_redirection(ast, *c),
        Token::T_SimpleCommand(vars, words) => vars.is_empty() && words.is_empty(),
        _ => false,
    }
}

pub fn is_function(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_Function(..))
}

/// Bats tests are functions for the purpose of 'local' and such
pub fn is_function_like(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_Function(..) | Token::T_BatsTest(..))
}

pub fn is_brace_expansion(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_BraceExpansion(..))
}

/// Get the lists of commands from tokens that contain them, such as
/// the conditions and bodies of while loops or branches of if statements.
pub fn get_command_sequences(ast: &Ast, t: Id) -> Vec<Vec<Id>> {
    match ast.get(t) {
        Token::T_Script(_, cmds) => vec![cmds.clone()],
        Token::T_BraceGroup(cmds) => vec![cmds.clone()],
        Token::T_Subshell(cmds) => vec![cmds.clone()],
        Token::T_WhileExpression(cond, cmds) => vec![cond.clone(), cmds.clone()],
        Token::T_UntilExpression(cond, cmds) => vec![cond.clone(), cmds.clone()],
        Token::T_ForIn(_, _, cmds) => vec![cmds.clone()],
        Token::T_ForArithmetic(_, _, _, cmds) => vec![cmds.clone()],
        Token::T_IfExpression(thens, elses) => {
            let mut out: Vec<Vec<Id>> = thens
                .iter()
                .flat_map(|(a, b)| [a.clone(), b.clone()])
                .collect();
            out.push(elses.clone());
            out
        }
        Token::T_Annotation(_, t) => get_command_sequences(ast, *t),

        Token::T_DollarExpansion(cmds) => vec![cmds.clone()],
        Token::T_DollarBraceCommandExpansion(_, cmds) => vec![cmds.clone()],
        Token::T_Backticked(cmds) => vec![cmds.clone()],
        _ => vec![],
    }
}

/// Get a list of names of associative arrays
pub fn get_associative_arrays(ast: &Ast, t: Id) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut f = |t: Id| {
        if !matches!(ast.get(t), Token::T_SimpleCommand(..)) {
            return;
        }
        let Some(name) = get_command_name(ast, t) else { return };
        if !["declare", "local", "typeset"].contains(&name.as_str()) {
            return;
        }
        let flags = get_all_flags(ast, t);
        if !flags.iter().any(|(_, s)| s == "A") {
            return;
        }
        let name_assignments = |t: Id| match ast.get(t) {
            Token::T_Assignment(_, name, _, _) => Some(name.clone()),
            _ => None,
        };
        for (arg, s) in &flags {
            if s.is_empty() {
                if let Some(n) = get_literal_string_ext(ast, &name_assignments, *arg) {
                    found.push(n);
                }
            }
        }
    };
    ast.analyze(t, &mut f, &mut |_| {});
    // nub
    let mut out: Vec<String> = Vec::new();
    for n in found {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// A Pseudoglob is a wildcard pattern used for checking if a match can succeed.
/// For example, [[ $(cmd).jpg == [a-z] ]] will give the patterns *.jpg and ?, which
/// can be proven never to match.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PseudoGlob {
    PGAny,
    PGMany,
    PGChar(char),
}
use PseudoGlob::*;

/// Turn a word into a PG pattern, replacing all unknown/runtime values with
/// PGMany.
pub fn word_to_pseudo_glob(ast: &Ast, word: Id) -> Vec<PseudoGlob> {
    word_to_pseudo_glob_(ast, false, word).unwrap_or_else(|| vec![PGMany])
}

/// Turn a word into a PG pattern, but only if we can preserve
/// exact semantics.
pub fn word_to_exact_pseudo_glob(ast: &Ast, word: Id) -> Option<Vec<PseudoGlob>> {
    word_to_pseudo_glob_(ast, true, word)
}

/// `wordToPseudoGlob'`
pub fn word_to_pseudo_glob_(ast: &Ast, exact: bool, word: Id) -> Option<Vec<PseudoGlob>> {
    let f = |x: Id| -> Option<Vec<PseudoGlob>> {
        match ast.get(x) {
            Token::T_Literal(s) => Some(s.chars().map(PGChar).collect()),
            Token::T_SingleQuoted(s) => Some(s.chars().map(PGChar).collect()),
            Token::T_Glob(s) if s == "?" => Some(vec![PGAny]),
            Token::T_Glob(s) if s == "*" => Some(vec![PGMany]),
            Token::T_Glob(s) if s.starts_with('[') && !exact => Some(vec![PGAny]),
            _ => {
                if exact { None } else { Some(vec![PGMany]) }
            }
        }
    };
    let map_f = |ids: Vec<Id>| -> Option<Vec<PseudoGlob>> {
        let mut out = Vec::new();
        for x in ids {
            out.extend(f(x)?);
        }
        Some(out)
    };
    let to_glob = || -> Option<Vec<PseudoGlob>> {
        if let Token::T_NormalWord(l) = ast.get(word) {
            if let Some((&first, rest)) = l.split_first() {
                if let Token::T_Literal(s) = ast.get(first) {
                    if let Some(str) = s.strip_prefix('~') {
                        if exact {
                            return None;
                        }
                        let mut this = vec![PGMany];
                        this.extend(str.chars().skip_while(|&c| c != '/').map(PGChar));
                        let parts: Vec<Id> =
                            rest.iter().flat_map(|&p| get_word_parts(ast, p)).collect();
                        this.extend(map_f(parts)?);
                        return Some(this);
                    }
                }
            }
        }
        map_f(get_word_parts(ast, word))
    };
    to_glob().map(|g| simplify_pseudo_glob(&g))
}

/// Reorder a PseudoGlob for more efficient matching, e.g.
/// f?*?**g -> f??*g
pub fn simplify_pseudo_glob(list: &[PseudoGlob]) -> Vec<PseudoGlob> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < list.len() {
        if let PGChar(_) = list[i] {
            out.push(list[i]);
            i += 1;
        } else {
            let n = list[i..]
                .iter()
                .take_while(|&&x| x == PGMany || x == PGAny)
                .count();
            let anys = &list[i..i + n];
            // order: all PGAny, then at most one PGMany
            out.extend(anys.iter().filter(|&&x| x == PGAny));
            out.extend(anys.iter().filter(|&&x| x != PGAny).take(1));
            i += n;
        }
    }
    out
}

/// Check whether the two patterns can ever overlap.
pub fn pseudo_globs_can_overlap(x: &[PseudoGlob], y: &[PseudoGlob]) -> bool {
    match (x, y) {
        ([xf, xs @ ..], [yf, ys @ ..]) => match (xf, yf) {
            (PGMany, _) => pseudo_globs_can_overlap(x, ys) || pseudo_globs_can_overlap(xs, y),
            (_, PGMany) => pseudo_globs_can_overlap(x, ys) || pseudo_globs_can_overlap(xs, y),
            (PGAny, _) => pseudo_globs_can_overlap(xs, ys),
            (_, PGAny) => pseudo_globs_can_overlap(xs, ys),
            (_, _) => xf == yf && pseudo_globs_can_overlap(xs, ys),
        },
        ([], []) => true,
        ([PGMany, rest @ ..], []) => pseudo_globs_can_overlap(rest, &[]),
        ([_, ..], []) => false,
        ([], r) => pseudo_globs_can_overlap(r, &[]),
    }
}

/// Check whether the first pattern always overlaps the second.
pub fn pseudo_glob_is_superset_of(x: &[PseudoGlob], y: &[PseudoGlob]) -> bool {
    match (x, y) {
        ([xf, xs @ ..], [yf, ys @ ..]) => match (xf, yf) {
            (PGMany, PGMany) => pseudo_glob_is_superset_of(x, ys),
            (PGMany, _) => pseudo_glob_is_superset_of(x, ys) || pseudo_glob_is_superset_of(xs, y),
            (_, PGMany) => false,
            (PGAny, _) => pseudo_glob_is_superset_of(xs, ys),
            (_, PGAny) => false,
            (_, _) => xf == yf && pseudo_glob_is_superset_of(xs, ys),
        },
        ([], []) => true,
        ([PGMany, rest @ ..], []) => pseudo_glob_is_superset_of(rest, &[]),
        _ => false,
    }
}

pub fn words_can_be_equal(ast: &Ast, x: Id, y: Id) -> bool {
    pseudo_globs_can_overlap(&word_to_pseudo_glob(ast, x), &word_to_pseudo_glob(ast, y))
}

/// Is this an expansion that can be quoted,
/// e.g. $(foo) `foo` $foo (but not {foo,})?
pub fn is_quoteable_expansion(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_DollarBraced(..) => true,
        _ => is_command_substitution(ast, t),
    }
}

pub fn is_command_substitution(ast: &Ast, t: Id) -> bool {
    matches!(
        ast.get(t),
        Token::T_DollarExpansion(..) | Token::T_DollarBraceCommandExpansion(..) | Token::T_Backticked(..)
    )
}

/// Is this an expansion that results in a simple string?
pub fn is_string_expansion(ast: &Ast, t: Id) -> bool {
    is_command_substitution(ast, t)
        || match ast.get(t) {
            Token::T_DollarArithmetic(..) => true,
            Token::T_DollarBraced(..) => !is_array_expansion(ast, t),
            _ => false,
        }
}

/// Is this a T_Annotation that ignores a specific code?
pub fn is_annotation_ignoring_code(ast: &Ast, code: i64, t: Id) -> bool {
    match ast.get(t) {
        Token::T_Annotation(anns, _) => anns.iter().any(|a| match a {
            Annotation::DisableComment(from, to) => code >= *from && code < *to,
            _ => false,
        }),
        _ => false,
    }
}

static ENV_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("/env +(-S|--split-string=?)? *(.*)"));

/// Get the shell executable from a string like '/usr/bin/env bash'
pub fn executable_from_shebang(s: &str) -> String {
    fn from_env_args(args: &[&str]) -> String {
        // skipFlags = dropWhile ("-" `isPrefixOf`)
        let skipped = args.iter().skip_while(|a| a.starts_with('-'));
        for a in skipped {
            if !a.contains('=') {
                return a.to_string();
            }
        }
        String::new()
    }
    if matches(s, &ENV_RE) {
        return match match_regex(&ENV_RE, s) {
            Some(g) if g.len() == 2 => {
                let words: Vec<&str> = g[1].split_whitespace().collect();
                from_env_args(&words)
            }
            _ => String::new(),
        };
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [x] => basename(x),
        [first, second, ..] if basename(first) == "busybox" => match basename(second).as_str() {
            "sh" => "busybox sh".to_string(),
            "ash" => "busybox ash".to_string(),
            x => x.to_string(),
        },
        [first, args @ ..] if basename(first) == "env" => from_env_args(args),
        [first, ..] => basename(first),
    }
}

// Determining if a name is a variable
pub fn is_variable_start_char(x: char) -> bool {
    x == '_' || x.is_ascii_lowercase() || x.is_ascii_uppercase()
}

pub fn is_variable_char(x: char) -> bool {
    is_variable_start_char(x) || x.is_ascii_digit()
}

pub fn is_special_variable_char(x: char) -> bool {
    "*@#?-$!".contains(x)
}

static VARIABLE_NAME_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("[_a-zA-Z][_a-zA-Z0-9]*"));

pub fn variable_name_regex() -> &'static Regex {
    &VARIABLE_NAME_RE
}

pub fn is_variable_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(x) => is_variable_start_char(x) && chars.all(is_variable_char),
        None => false,
    }
}

/// Get the variable name from an expansion like ${var:-foo}
pub fn get_braced_reference(s: &str) -> String {
    let drop_prefix = |s: &str| -> String {
        match s.chars().next() {
            Some(c) if c == '!' || c == '#' => s[c.len_utf8()..].to_string(),
            _ => s.to_string(),
        }
    };
    let take_name = |s: &str| -> Option<String> {
        let name: String = s.chars().take_while(|&c| is_variable_char(c)).collect();
        if name.is_empty() { None } else { Some(name) }
    };
    let get_special = |s: &str| -> Option<String> {
        match s.chars().next() {
            Some(c) if is_special_variable_char(c) => Some(c.to_string()),
            _ => None, // empty or not special
        }
    };
    let name_expansion = |s: &str| -> Option<String> {
        // e.g. ${!foo*bar*}
        let mut it = s.chars();
        if it.next() != Some('!') {
            return None;
        }
        let next = it.next()?;
        if !is_variable_char(next) {
            return None; // e.g. ${!@}
        }
        let first = it.find(|&c| !is_variable_char(c))?;
        if "*?@".contains(first) { Some(String::new()) } else { None }
    };
    let no_prefix = drop_prefix(s);
    name_expansion(s)
        .or_else(|| take_name(&no_prefix))
        .or_else(|| get_special(&no_prefix))
        .or_else(|| get_special(s))
        .unwrap_or_else(|| s.to_string())
}

/// Get the variable modifier like /a/b in ${var/a/b}
pub fn get_braced_modifier(s: &str) -> String {
    fn drop_prefix(var: &str, t: &str) -> Option<String> {
        // dropPrefix [] t = return t; mismatch = []
        t.strip_prefix(var).map(|r| r.to_string())
    }
    let var = get_braced_reference(s);
    let candidates: Vec<String> = match s.chars().next() {
        Some(c) if c == '#' || c == '!' => vec![s[c.len_utf8()..].to_string(), s.to_string()],
        _ => vec![s.to_string()],
    };
    // headOrDefault "" over the list monad
    candidates
        .iter()
        .find_map(|a| drop_prefix(&var, a))
        .unwrap_or_default()
}

/// Get the variables from indices like ["x", "y"] in ${var[x+y+1]}
pub fn get_index_references(s: &str) -> Vec<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("(\\[.*\\])"));
    match match_regex(&RE, s) {
        Some(g) if !g.is_empty() => match_all_strings(variable_name_regex(), &g[0]),
        _ => vec![],
    }
}

pub fn get_offset_references(mods: &str) -> Vec<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^(\\[.+\\])? *:([^-=?+].*)"));
    // if mods start with [, then drop until ]
    match match_regex(&RE, mods) {
        Some(g) if g.len() >= 2 => match_all_strings(variable_name_regex(), &g[1]),
        _ => vec![],
    }
}

/// Returns whether a token is a parameter expansion without any modifiers.
/// True for $var ${var} $1 $#
/// False for ${#var} ${var[x]} ${var:-0}
pub fn is_unmodified_parameter_expansion(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_DollarBraced(false, _) => true,
        Token::T_DollarBraced(_, list) => {
            let str = oversimplify(ast, *list).concat();
            get_braced_reference(&str) == str
        }
        _ => false,
    }
}

/// Return the referenced variable if (and only if) it's an unmodified parameter expansion.
pub fn get_unmodified_parameter_expansion(ast: &Ast, t: Id) -> Option<String> {
    match ast.get(t) {
        Token::T_DollarBraced(_, list) => {
            let str = oversimplify(ast, *list).concat();
            if get_braced_reference(&str) == str { Some(str) } else { None }
        }
        _ => None,
    }
}

/// A list of the element and all its parents up to the root node.
// DIVERGENCE: Haskell takes the parent `Map Id Token`; here the parents
// recorded in the arena by `Ast::compute_parents` are used, and the
// `NonEmpty` result is a `Vec` that always has at least one element.
pub fn get_path(ast: &Ast, t: Id) -> Vec<Id> {
    let mut out = vec![t];
    let mut cur = t;
    while let Some(p) = ast.parent(cur) {
        out.push(p);
        cur = p;
    }
    out
}

pub fn is_closing_file_op(ast: &Ast, op: Id) -> bool {
    match ast.get(op) {
        Token::T_IoDuplicate(fd, s) if s == "-" => {
            matches!(ast.get(*fd), Token::T_GREATAND | Token::T_LESSAND)
        }
        _ => false,
    }
}

pub fn get_enable_directives(ast: &Ast, root: Id) -> Vec<String> {
    match ast.get(root) {
        Token::T_Annotation(list, _) => list
            .iter()
            .filter_map(|a| match a {
                Annotation::EnableComment(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

pub fn get_extended_analysis_directive(ast: &Ast, root: Id) -> Option<bool> {
    match ast.get(root) {
        Token::T_Annotation(list, _) => list.iter().find_map(|a| match a {
            Annotation::ExtendedAnalysis(s) => Some(*s),
            _ => None,
        }),
        _ => None,
    }
}
