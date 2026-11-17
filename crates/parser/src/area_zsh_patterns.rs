// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only. zsh patterns and subscripts (no ShellCheck
// counterpart). Every entry point here is reached only after an
// `env.is_zsh()` check in the shared parser files, so other shells parse
// exactly as before.
//
// What this covers (research constructs):
// - C04 / C26: parenthesised pattern groups `(a|b)`, `(#b)`/`(#i)`/`(#m)`/`(#q)`
//   flags, and `#` / `##` repeats after them, in `[[ ]]` operands, case
//   patterns and ordinary words.
// - C05: numeric ranges `<->`, `<1->`, `<0-65535>`.
// - C06: glob qualifiers after a word, `x(N)`, `**/x(-.N:h)`.
// - C07: case labels that start with a group, `(a|b)*)`, `<->)`.
// - C17 / C27: unbraced subscripts `$a[i]`, `$a[(I)x]`, `$a[(s: :w)2]`.
// - C21: balanced `{ }` inside a `${ }` word, `${:-{$#p..1}}`.
//
// AST: no new variants. A group is `T_Extglob("", parts)` (the nested-group
// shape `read_extglob_part` already builds), a numeric range is a `T_Glob`,
// and a subscripted `$a[..]` is `T_DollarBraced(true, word)` shaped like the
// braced `${a[..]}` (name and brackets in the literals, expansions inside the
// subscript as their own parts), so name lookups see the same reference.

use shelllint_core::ast::{Id, Token};
use shelllint_core::interface::Severity::ErrorC;

use crate::engine::{P, PR};
use crate::state::IncompleteInterval;
use crate::{alt, choice};

/// The chars that end a literal run inside a subscript.
const SUBSCRIPT_STOP: &str = "[]$`'\"\\";

impl<'a> P<'a> {
    /// The next `n` characters of input, without consuming.
    fn zsh_peek(&mut self, n: usize) -> Vec<char> {
        let inp = self.get_input();
        inp.as_slice().iter().take(n).copied().collect()
    }

    /// `readNormalishWord` for zsh: the same word, but a `(` after a part
    /// (glob qualifier or group) or at the start of a pattern word (`[[ ]]`
    /// operand, case label) is a pattern group, and `<->` is a glob.
    pub(crate) fn zsh_normalish_word(&mut self, end: &str, terms: &[&str], pattern: bool) -> PR<Id> {
        let start = self.start_span();
        let pos = self.get_position();
        let mut count = 0usize;
        let mut last: Option<Id> = None;
        let x = self.many1(|p| {
            let at_start = count == 0;
            let after_eq = last.is_some_and(|l| matches!(p.user.ast.get(l), Token::T_Literal(s) if s.ends_with('=')));
            let id = p.zsh_word_part(end, at_start, pattern, after_eq)?;
            count += 1;
            last = Some(id);
            Ok(id)
        })?;
        let id = self.end_span(start);
        self.check_possible_termination(pos, &x, terms);
        Ok(self.node(id, Token::T_NormalWord(x)))
    }

    fn zsh_word_part(&mut self, end: &str, at_start: bool, pattern: bool, after_eq: bool) -> PR<Id> {
        let ahead = self.zsh_peek(2);
        let c0 = ahead.first().copied();
        if c0 == Some('(') && !end.contains('(') {
            // `x=(..)` is an array value and `=(..)` a process substitution:
            // not ours.
            let flag = ahead.get(1) == Some(&'#');
            let group_ok = if at_start { pattern || flag } else { !after_eq };
            if group_ok && let Some(g) = self.option_maybe(|p| p.zsh_group())? {
                return Ok(g);
            }
        }
        if c0 == Some('<')
            && let Some(r) = self.option_maybe(|p| p.zsh_numeric_range())?
        {
            return Ok(r);
        }
        self.read_normal_word_part(end)
    }

    /// `( parts | parts ... )`, at least one character inside.
    pub(crate) fn zsh_group(&mut self) -> PR<Id> {
        self.try_(|p| {
            let start = p.start_span();
            p.char_('(')?;
            p.not_followed_by2(|p| p.char_(')'))?;
            let contents = p.sep_by(|p| p.read_extglob_part(), |p| p.char_('|'))?;
            p.char_(')')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Extglob(String::new(), contents)))
        })
    }

    /// zsh: `[[ $a[1] == \] ]]`, a word that is just an escaped `]`, is not a
    /// missing space before `]]`. (The AST keeps the unescaped `]`.)
    pub(crate) fn zsh_escaped_bracket_word(&self, word: Id) -> bool {
        self.env.is_zsh()
            && matches!(self.user.ast.get(word), Token::T_NormalWord(parts)
                if parts.len() == 1 && matches!(self.user.ast.get(parts[0]), Token::T_Literal(s) if s == "]"))
    }

    /// `<->`, `<3->`, `<-9>`, `<0-65535>`.
    fn zsh_numeric_range(&mut self) -> PR<Id> {
        self.try_(|p| {
            let start = p.start_span();
            p.char_('<')?;
            let lo: String = p.many(|p| p.digit())?.into_iter().collect();
            p.char_('-')?;
            let hi: String = p.many(|p| p.digit())?.into_iter().collect();
            p.char_('>')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Glob(format!("<{lo}-{hi}>"))))
        })
    }

    /// The optional `(` before a case label. In zsh `(a|b)*)` is one pattern,
    /// not an opening paren followed by `a|b`: the `(` is the optional one
    /// only when the text after its matching `)` ends the label (blank,
    /// newline, `;`, end of input).
    pub(crate) fn zsh_case_lparen(&mut self) -> PR<Id> {
        let inp = self.get_input();
        let rest = inp.as_slice();
        if rest.first() != Some(&'(') {
            return self.zero();
        }
        let mut depth = 0i32;
        let mut i = 0usize;
        let mut after = None;
        while i < rest.len() {
            match rest[i] {
                '\\' => i += 1,
                '\'' => {
                    i += 1;
                    while i < rest.len() && rest[i] != '\'' {
                        i += 1;
                    }
                }
                '"' => {
                    i += 1;
                    while i < rest.len() && rest[i] != '"' {
                        if rest[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        after = rest.get(i + 1).copied();
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let optional_paren = match after {
            None => true,
            Some(c) => c.is_whitespace() || c == ';' || c == '&',
        };
        if optional_paren { self.g_lparen() } else { self.zero() }
    }

    /// `$name[subscript]`; `start` is the position of the `$`. `None` when no
    /// `[` follows the name. A `[` whose subscript never closes is a parse
    /// error in zsh ("invalid subscript"): it is reported (SC1073) and `None`
    /// is returned so the caller falls back to the plain variable.
    pub(crate) fn zsh_subscripted_variable(&mut self, start: IncompleteInterval) -> PR<Option<Id>> {
        if let Some(id) = self.option_maybe(|p| p.try_(|p| p.zsh_subscripted_inner(start)))? {
            return Ok(Some(id));
        }
        let unclosed = self
            .try_(|p| {
                p.look_ahead(|p| {
                    p.read_variable_name()?;
                    p.char_('[')
                })
            })
            .is_ok();
        if unclosed {
            self.parse_problem_at(start.0, ErrorC, 1073, "Couldn't parse this subscript. Is the closing ] missing?");
        }
        Ok(None)
    }

    fn zsh_subscripted_inner(&mut self, start: IncompleteInterval) -> PR<Id> {
        let word_start = self.start_span();
        let name = self.read_variable_name()?;
        self.look_ahead(|p| p.char_('['))?;
        let mut parts: Vec<Id> = Vec::new();
        let mut text = name;
        self.zsh_subscript_into(&mut parts, &mut text)?;
        // `text` holds the trailing literal; the first literal carries the name.
        self.zsh_flush_literal(&mut parts, &mut text, word_start);
        let wid = self.end_span(word_start);
        let word = self.node(wid, Token::T_NormalWord(parts));
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_DollarBraced(true, word)))
    }

    /// Reads `[ ... ]` (balanced), pushing expansions as parts and the rest as
    /// literal text; `text` is flushed into a literal before each expansion.
    fn zsh_subscript_into(&mut self, parts: &mut Vec<Id>, text: &mut String) -> PR<()> {
        self.char_('[')?;
        text.push('[');
        let mut depth = 1usize;
        loop {
            let seg_start = self.start_span();
            let chunk: String = self.many(|p| p.none_of(SUBSCRIPT_STOP))?.into_iter().collect();
            text.push_str(&chunk);
            let ahead = self.zsh_peek(1);
            match ahead.first().copied() {
                Some(']') => {
                    self.char_(']')?;
                    text.push(']');
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some('[') => {
                    self.char_('[')?;
                    text.push('[');
                    depth += 1;
                }
                Some('\\') => {
                    let b = self.char_('\\')?;
                    let c = self.any_char()?;
                    text.push(b);
                    text.push(c);
                }
                Some('$') | Some('`') | Some('\'') | Some('"') => {
                    self.zsh_flush_literal(parts, text, seg_start);
                    let id = choice!(
                        self,
                        |p: &mut Self| p.read_single_quoted(),
                        |p: &mut Self| p.read_double_quoted(),
                        |p: &mut Self| p.read_normal_dollar(),
                        |p: &mut Self| p.read_unquoted_back_ticked(),
                    )?;
                    parts.push(id);
                }
                _ => return self.fail("Unterminated subscript"),
            }
        }
    }

    /// Turns pending literal text into a `T_Literal` part.
    fn zsh_flush_literal(&mut self, parts: &mut Vec<Id>, text: &mut String, from: IncompleteInterval) {
        if text.is_empty() {
            return;
        }
        let id = self.end_span(from);
        let s = std::mem::take(text);
        parts.push(self.node(id, Token::T_Literal(s)));
    }

    /// A balanced `{ ... }` inside `${ }`: `${:-{$#p..1}}`. Falls back to a
    /// lone `{` literal when the group does not close.
    pub(crate) fn zsh_braced_nested(&mut self) -> PR<Id> {
        let nested = |p: &mut Self| {
            p.try_(|p| {
                let start = p.start_span();
                let open = p.zsh_lit_char('{')?;
                let mut parts = vec![open];
                parts.extend(p.many(|p| p.read_dollar_braced_part())?);
                parts.push(p.zsh_lit_char('}')?);
                let id = p.end_span(start);
                Ok(p.node(id, Token::T_NormalWord(parts)))
            })
        };
        alt!(self, nested, |p: &mut Self| p.zsh_lit_char('{'))
    }

    fn zsh_lit_char(&mut self, c: char) -> PR<Id> {
        let start = self.start_span();
        self.char_(c)?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(c.to_string())))
    }
}
