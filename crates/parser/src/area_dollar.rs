// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1612-1835:
// all $ expansions, variable names.
// Some readers here are only called from other areas' readers, which may still
// be stubs.

use shellcheck_core::ast::{Id, Piped, Token};
use shellcheck_core::interface::Severity;

use crate::alt;
use crate::engine::{P, PR, SourcePos};

impl<'a> P<'a> {
    // The grammar should have been designed along the lines of readDollarExpr = char '$' >> stuff, but
    // instead, each subunit parses its own $. This results in ~7 1-3 char lookaheads instead of one 1-char.
    // Instead of optimizing the grammar, here's a green cut that decreases shellcheck runtime by 10%:
    pub(crate) fn ensure_dollar(&mut self) -> PR<char> {
        self.look_ahead(|p| p.char_('$'))
    }

    pub(crate) fn read_normal_dollar(&mut self) -> PR<Id> {
        self.ensure_dollar()?;
        alt!(
            self,
            |p| p.read_dollar_exp(),
            |p| p.read_dollar_double_quote(),
            |p| p.read_dollar_single_quote(),
            |p| p.read_dollar_lonely(false),
        )
    }

    pub(crate) fn read_double_quoted_dollar(&mut self) -> PR<Id> {
        self.ensure_dollar()?;
        alt!(self, |p| p.read_dollar_exp(), |p| p.read_dollar_lonely(true))
    }

    pub(crate) fn read_dollar_expression(&mut self) -> PR<Id> {
        self.ensure_dollar()?;
        self.read_dollar_exp()
    }

    pub(crate) fn read_dollar_exp(&mut self) -> PR<Id> {
        // where
        let arithmetic = |p: &mut Self| {
            p.read_ambiguous(
                "$((",
                |p| p.read_dollar_arithmetic(),
                |p| p.read_dollar_expansion(),
                |p, pos| {
                    p.parse_note_at(
                        pos,
                        Severity::ErrorC,
                        1102,
                        "Shells disambiguate $(( differently or not at all. For $(command substitution), add space after $( . For $((arithmetics)), fix parsing errors.",
                    );
                    Ok(())
                },
            )
        };
        alt!(
            self,
            arithmetic,
            |p| p.read_dollar_expansion(),
            |p| p.read_dollar_bracket(),
            |p| p.read_dollar_brace_command_expansion(),
            |p| p.read_dollar_braced(),
            |p| p.read_dollar_variable(),
        )
    }

    pub(crate) fn read_dollar_single_quote(&mut self) -> PR<Id> {
        self.called("$'..' expression", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("$'"))?;
            let str = p.read_generic_literal("'")?;
            p.char_('\'')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarSingleQuoted(str)))
        })
    }

    pub(crate) fn read_dollar_double_quote(&mut self) -> PR<Id> {
        self.look_ahead(|p| p.try_(|p| p.string("$\"")))?;
        let start = self.start_span();
        self.char_('$')?;
        self.double_quote()?;
        let x = self.many(|p| p.double_quoted_part())?;
        self.alt(|p| p.double_quote(), |p| p.fail("Expected end of translated double quoted string"))?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_DollarDoubleQuoted(x)))
    }

    pub(crate) fn read_dollar_arithmetic(&mut self) -> PR<Id> {
        self.called("$((..)) expression", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("$(("))?;
            let c = p.read_arithmetic_contents()?;
            let _pos = p.get_position();
            p.char_(')')?;
            p.alt(|p| p.char_(')'), |p| p.fail("Expected a double )) to end the $((..))"))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarArithmetic(c)))
        })
    }

    pub(crate) fn read_dollar_bracket(&mut self) -> PR<Id> {
        self.called("$[..] expression", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("$["))?;
            let c = p.read_arithmetic_contents()?;
            p.string("]")?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarBracket(c)))
        })
    }

    pub(crate) fn read_arithmetic_expression(&mut self) -> PR<Id> {
        self.called("((..)) command", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("(("))?;
            let c = p.read_arithmetic_contents()?;
            p.string("))")?;
            let id = p.end_span(start);
            p.spacing()?;
            Ok(p.node(id, Token::T_Arithmetic(c)))
        })
    }

    /// If the next characters match prefix, try two different parsers and warn if the alternate parser had to be used
    pub(crate) fn read_ambiguous<T>(
        &mut self,
        prefix: &str,
        expected: impl FnMut(&mut Self) -> PR<T>,
        alternative: impl FnMut(&mut Self) -> PR<T>,
        warner: impl FnMut(&mut Self, SourcePos) -> PR<()>,
    ) -> PR<T> {
        let pos = self.get_position();
        self.try_(|p| p.look_ahead(|p| p.string(prefix)))?;
        // If the expected parser fails, try the alt.
        // If the alt fails, run the expected one again for the errors.
        // `try expected <|> try (withAlt pos) <|> expected`; `alt_with`
        // shares the three parsers between the branches.
        let mut st = (expected, alternative, warner);
        self.alt_with(
            &mut st,
            |p, st| p.try_(|p| (st.0)(p)),
            |p, st| {
                p.alt_with(
                    st,
                    // withAlt pos
                    |p, st| {
                        p.try_(|p| {
                            let t = p.forget_on_failure(|p| (st.1)(p))?;
                            (st.2)(p, pos)?;
                            Ok(t)
                        })
                    },
                    |p, st| (st.0)(p),
                )
            },
        )
    }

    pub(crate) fn read_dollar_brace_command_expansion(&mut self) -> PR<Id> {
        self.called("ksh-style ${ ..; } command expansion", |p| {
            let start = p.start_span();
            let c = p.try_(|p| {
                p.string("${")?;
                p.alt(|p| p.char_('|'), |p| p.whitespace())
            })?;
            p.allspacing()?;
            let term = p.read_term()?;
            p.alt(
                |p| p.char_('}'),
                |p| p.fail("Expected } to end the ksh-style ${ ..; } command expansion"),
            )?;
            let id = p.end_span(start);
            let piped = if c == '|' { Piped::Piped } else { Piped::Unpiped };
            Ok(p.node(id, Token::T_DollarBraceCommandExpansion(piped, term)))
        })
    }

    pub(crate) fn read_dollar_braced(&mut self) -> PR<Id> {
        self.called("parameter expansion", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("${"))?;
            let word = p.read_dollar_braced_word()?;
            p.char_('}')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarBraced(true, word)))
        })
    }

    pub(crate) fn read_dollar_expansion(&mut self) -> PR<Id> {
        self.called("command expansion", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("$("))?;
            let cmds = p.read_compound_list_or_empty()?;
            p.alt(|p| p.char_(')'), |p| p.fail("Expected end of $(..) expression"))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarExpansion(cmds)))
        })
    }

    pub(crate) fn read_dollar_variable(&mut self) -> PR<Id> {
        let start = self.start_span();
        let pos = self.get_position();

        let single_charred = move |p: &mut Self, c: fn(&mut Self) -> PR<char>| -> PR<Id> {
            let value = p.wrap_string(|p| c(p).map(String::from))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_DollarBraced(false, value)))
        };

        let positional = |p: &mut Self| -> PR<Id> {
            let value = single_charred(p, |p| p.digit())?;
            p.attempting(
                |_| Ok(value),
                |p| {
                    p.look_ahead(|p| p.digit())?;
                    p.parse_note_at(pos, Severity::ErrorC, 1037, "Braces are required for positionals over 9, e.g. ${10}.");
                    Ok(())
                },
            )
        };

        let special = |p: &mut Self| single_charred(p, |p| p.special_variable());

        let regular = |p: &mut Self| -> PR<Id> {
            let value = p.wrap_string(|p| p.read_variable_name())?;
            let id = p.end_span(start);
            let value = p.node(id, Token::T_DollarBraced(false, value));
            p.attempting(
                |_| Ok(value),
                |p| {
                    p.look_ahead(|p| p.char_('['))?;
                    p.parse_note_at(
                        pos,
                        Severity::ErrorC,
                        1087,
                        "Use braces when expanding arrays, e.g. ${array[idx]} (or ${var}[.. to quiet).",
                    );
                    Ok(())
                },
            )
        };

        self.try_(|p| {
            p.char_('$')?;
            alt!(p, positional, special, regular)
        })
    }

    /// `wrapString` from readDollarVariable's `where` clause
    fn wrap_string(&mut self, p: impl FnOnce(&mut Self) -> PR<String>) -> PR<Id> {
        let start = self.get_position();
        let s = p(self)?;
        let end = self.get_position();
        let id1 = self.get_next_id_between(start, end);
        let id2 = self.get_next_id_between(start, end);
        self.node(id2, Token::T_Literal(s));
        Ok(self.node(id1, Token::T_NormalWord(vec![id2])))
    }

    pub(crate) fn read_variable_name(&mut self) -> PR<String> {
        let f = self.variable_start()?;
        let rest = self.many(|p| p.variable_chars())?;
        Ok(std::iter::once(f).chain(rest).collect())
    }

    pub(crate) fn read_dollar_lonely(&mut self, quoted: bool) -> PR<Id> {
        let start = self.start_span();
        self.char_('$')?;
        let id = self.end_span(start);
        if quoted {
            let is_hack = self.quote_for_escape()?;
            if is_hack {
                self.parse_problem_at_id(
                    id,
                    Severity::StyleC,
                    1135,
                    "Prefer escape over ending quote to make $ literal. Instead of \"It costs $\"5, use \"It costs \\$5\".",
                );
            }
        }
        Ok(self.node(id, Token::T_Literal("$".to_string())))
    }

    /// `quoteForEscape` from readDollarLonely's `where` clause
    fn quote_for_escape(&mut self) -> PR<bool> {
        self.option(false, |p| {
            p.try_(|p| {
                p.look_ahead(|p| {
                    p.char_('"')?;
                    // Check for "foo $""bar"
                    p.optional(|p| p.char_('"'))?;
                    let c = p.any_var()?;
                    // Don't trigger on [[ x == "$"* ]] or "$"$pattern
                    Ok(!"*$".contains(c))
                })
            })
        })
    }

    /// `anyVar` from readDollarLonely's `where` clause
    fn any_var(&mut self) -> PR<char> {
        alt!(self, |p| p.variable_start(), |p| p.digit(), |p| p.special_variable())
    }
}
