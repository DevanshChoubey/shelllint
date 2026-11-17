// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only (zsh support, lane P-C). ShellCheck has no zsh
// dialect; every reader here is reached only through a hook that checks
// `Environment::is_zsh`, so the parse of every other shell is unchanged.
//
// The remaining zsh syntax that is not about functions or patterns:
//   * `for x (list)`, `for k v in list`, and the SHORT_LOOPS one-command
//     bodies of `for` loops, with or without a `;` or a newline before them;
//   * the `[##16]` / `[#16]` output-base prefix and `f()` function calls in
//     arithmetic;
//   * the `>>|` / `>>!` / `>!` forced redirections, and `{1}>&-` (a numeric
//     fd variable) in front of a redirection;
//   * an `if` without `then` that is closed by `fi`, `}` or `)` (gallois-style
//     `if cond` NEWLINE command, accepted by zsh because SHORT_LOOPS is on);
//   * the SC1083 exemption for prompt escapes (`%{ %}`, `%F{blue}`) and
//     `@{u}` revision suffixes.

use shelllint_core::ast::{Id, Token};

use crate::engine::{P, PR};
use crate::{alt, choice};

impl<'a> P<'a> {
    // ---- for loops --------------------------------------------------------

    /// The part of a `for` loop after the keyword, when it is a `for name ...`
    /// form (not the arithmetic form). zsh additionally allows more than one
    /// name (`for k v in ...`), a parenthesised word list (`for x (a b)`), and
    /// a loop body that is a single command without `do ... done`.
    pub(crate) fn zsh_for_regular(&mut self, id: Id) -> PR<Id> {
        self.accept_but_warn(
            |p| p.char_('$'),
            shelllint_core::interface::Severity::ErrorC,
            1086,
            "Don't use $ on the iterator name in for loops.",
        )?;
        let name = self.then_skip(|p| p.zsh_for_name(), |p| p.allspacing())?;
        let extra = self.zsh_for_extra_names()?;
        let values =
            choice!(self, |p: &mut Self| p.read_in_clause(), |p: &mut Self| p.zsh_for_paren_list(), |p: &mut Self| {
                p.optional(|p| p.read_sequential_sep())?;
                Ok(vec![])
            },)?;
        let mut group = self.zsh_loop_body(id)?;
        // DIVERGENCE: `T_ForIn` holds one name. Each further name is modelled
        // as a nested `for name in; ...` with no words, which assigns it for
        // the analysis (like a `for` over unknown input) without a new token.
        for (pos, end, extra_name) in extra.into_iter().rev() {
            let eid = self.get_next_id_between(pos, end);
            let empty = self.node(eid, Token::T_ForIn(extra_name, vec![], vec![]));
            group.insert(0, empty);
        }
        Ok(self.node(id, Token::T_ForIn(name, values, group)))
    }

    /// A loop variable: an identifier, or digits (`for 1 2 3; do ...` loops
    /// over the positional parameters).
    fn zsh_for_name(&mut self) -> PR<String> {
        self.alt(
            |p| p.read_variable_name(),
            |p| {
                p.try_(|p| {
                    let ds = p.many1(|p| p.digit())?;
                    p.not_followed_by2(|p| p.variable_chars())?;
                    Ok(ds.into_iter().collect())
                })
            },
        )
    }

    /// `k v` of `for k v in ...` / `for k v (...)` / `for k v; do`: more names,
    /// which must be followed by `in`, `do`, `(`, `{`, `;` or a newline.
    #[allow(clippy::type_complexity)]
    fn zsh_for_extra_names(&mut self) -> PR<Vec<(crate::engine::SourcePos, crate::engine::SourcePos, String)>> {
        self.alt(
            |p| {
                p.try_(|p| {
                    let mut names = Vec::new();
                    loop {
                        let step = p.option_maybe(|p| {
                            p.not_followed_by2(|p| p.g_in())?;
                            p.not_followed_by2(|p| p.g_do())?;
                            let start = p.get_position();
                            let n = p.zsh_for_name()?;
                            let end = p.get_position();
                            p.allspacing()?;
                            Ok((start, end, n))
                        })?;
                        match step {
                            Some(n) => names.push(n),
                            None => break,
                        }
                    }
                    if names.is_empty() {
                        return p.fail("no more loop names");
                    }
                    p.look_ahead(|p| {
                        alt!(p, |p| p.g_in().map(drop), |p| p.g_do().map(drop), |p| p.one_of("(;{\n").map(drop))
                    })?;
                    Ok(names)
                })
            },
            |_| Ok(Vec::new()),
        )
    }

    /// `(a b c)`, the word list of `for x (a b c)`; may span lines.
    fn zsh_for_paren_list(&mut self) -> PR<Vec<Id>> {
        self.try_(|p| {
            p.char_('(')?;
            p.allspacing()?;
            let words = p.many(|p| {
                let w = p.read_normal_word()?;
                p.allspacing()?;
                Ok(w)
            })?;
            p.char_(')')?;
            p.spacing()?;
            p.optional(|p| p.read_sequential_sep())?;
            Ok(words)
        })
    }

    /// The body of a zsh loop: `{ ... }`, `do ... done`, or (SHORT_LOOPS) one
    /// command. The error for none of them is the usual "Expected 'do'".
    pub(crate) fn zsh_loop_body(&mut self, id: Id) -> PR<Vec<Id>> {
        self.alt(
            |p| {
                let group = p.read_brace_group()?;
                match p.user.ast.get(group) {
                    Token::T_BraceGroup(list) => Ok(list.clone()),
                    _ => unreachable!("readBraceGroup returns a T_BraceGroup"),
                }
            },
            |p| {
                if p.is_followed_by(|p| p.g_do())? {
                    return p.read_do_group(id);
                }
                p.alt(
                    |p| {
                        p.allspacing()?;
                        let cmd = p.read_and_or()?;
                        Ok(vec![cmd])
                    },
                    |p| p.read_do_group(id),
                )
            },
        )
    }

    // ---- if without then --------------------------------------------------

    /// `if cond` NEWLINE ... closed by `fi`, `}` or `)` instead of `then`.
    /// zsh accepts this (SHORT_LOOPS); the condition list runs to the closer,
    /// which is left for the enclosing construct. Fails without consuming
    /// anything when the shape is not that.
    pub(crate) fn zsh_if_without_then(&mut self) -> PR<Id> {
        self.try_(|p| {
            let start = p.start_span();
            p.g_if()?;
            p.allspacing()?;
            let condition = p.read_term()?;
            p.allspacing()?;
            p.not_followed_by2(|p| p.g_then())?;
            p.look_ahead(|p| alt!(p, |p| p.g_fi().map(drop), |p| p.g_rbrace().map(drop), |p| p.char_(')').map(drop)))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_IfExpression(vec![(condition, vec![])], vec![])))
        })
    }

    // ---- arithmetic -------------------------------------------------------

    /// `[##16] expr` / `[#16] expr`: the output base prefix.
    pub(crate) fn zsh_arith_base_prefix(&mut self) -> PR<Id> {
        let start = self.start_span();
        let text = self.try_(|p| {
            p.string("[#")?;
            let hash = p.option(String::new(), |p| p.char_('#').map(|c| c.to_string()))?;
            let base = p.many1(|p| p.satisfy(|c| c != ']' && !c.is_whitespace()))?;
            p.char_(']')?;
            Ok(format!("[#{hash}{}]", base.into_iter().collect::<String>()))
        })?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        let x = self.arith_read_any_negated()?;
        Ok(self.node(id, Token::TA_Unary(text, x)))
    }

    /// `name(args)`: a math function call, as in `(( rand48() < 2 ))`.
    // DIVERGENCE: modelled as a unary operator named `name()` on the
    // parenthesised arguments, so the arguments are still analysed and the
    // function name is not taken for a variable read.
    pub(crate) fn zsh_arith_call(&mut self) -> PR<Id> {
        let start = self.start_span();
        let name = self.try_(|p| {
            let n = p.read_variable_name()?;
            p.char_('(')?;
            Ok(n)
        })?;
        let args_start = self.start_span();
        self.arith_spacing()?;
        let args = self.arith_sequence()?;
        self.char_(')')?;
        let paren = self.end_span(args_start);
        let paren = self.node(paren, Token::TA_Parenthesis(args));
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok(self.node(id, Token::TA_Unary(format!("{name}()"), paren)))
    }

    // ---- redirections -----------------------------------------------------

    /// `>>|`, `>>!` (force append) and `>!` (force truncate).
    pub(crate) fn zsh_forced_redirect_op(&mut self) -> PR<Id> {
        self.try_(|p| {
            let start = p.start_span();
            let append = p.alt(|p| p.string(">>").map(|_| true), |p| p.char_('>').map(|_| false))?;
            p.one_of("|!")?;
            let id = p.end_span(start);
            p.not_followed_by2(|p| p.char_('('))?;
            Ok(if append { p.node(id, Token::T_DGREAT) } else { p.node(id, Token::T_CLOBBER) })
        })
    }

    /// The name inside `{...}` before a redirection: an identifier, or digits
    /// (zsh reads `{1}>&-` as "the descriptor held in parameter 1").
    pub(crate) fn zsh_io_variable_name(&mut self) -> PR<String> {
        self.alt(|p| p.read_variable_name(), |p| p.many1(|p| p.digit()).map(|ds| ds.into_iter().collect()))
    }

    // ---- SC1083 -----------------------------------------------------------

    /// Is the literal `{` or `}` just consumed (it is the last character
    /// before the cursor) part of a prompt escape or a revision suffix, which
    /// zsh users write unquoted: `%{`, `%}`, `%F{blue}`, `@{u}`?
    pub(crate) fn zsh_is_prompt_brace(&mut self, c: char) -> bool {
        let input = self.get_input();
        let before = input.preceding();
        let Some((_, before)) = before.split_last() else { return false };
        match c {
            '{' => opens_prompt_form(before),
            _ => {
                if before.last() == Some(&'%') {
                    return true;
                }
                // `%F{blue}`, `@{u}`, `%D{"%H:%M"}`: walk back over the
                // argument (quoted chunks included) to its `{`.
                let mut i = before.len();
                while i > 0 {
                    let ch = before[i - 1];
                    if ch == '"' || ch == '\'' {
                        match before[..i - 1].iter().rposition(|&q| q == ch) {
                            Some(open) => i = open,
                            None => return false,
                        }
                    } else if ch == '{' {
                        return opens_prompt_form(&before[..i - 1]);
                    } else if ch == '}' || ch.is_whitespace() {
                        return false;
                    } else {
                        i -= 1;
                    }
                }
                false
            }
        }
    }
}

/// `before` is the text in front of a `{`: ends in `%`, `@`, or `%` followed
/// by digits and a letter (`%F`, `%K`, `%10F`).
fn opens_prompt_form(before: &[char]) -> bool {
    match before.last() {
        Some('%' | '@') => true,
        Some(c) if c.is_ascii_alphabetic() => {
            let mut i = before.len() - 1;
            while i > 0 && before[i - 1].is_ascii_digit() {
                i -= 1;
            }
            i > 0 && before[i - 1] == '%'
        }
        _ => false,
    }
}
