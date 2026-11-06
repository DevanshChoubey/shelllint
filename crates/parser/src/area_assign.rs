// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2909-3127:
// command prefixes/suffixes (let/eval/time/declare), assignments, arrays,
// token helpers.

use shellcheck_core::ast::{AssignmentMode, Id, Token};
use shellcheck_core::interface::Severity::{ErrorC, WarningC};

use crate::engine::{P, PR, SourcePos, update_pos_char};
use crate::{alt, choice};

impl<'a> P<'a> {
    pub(crate) fn read_cmd_prefix(&mut self) -> PR<Vec<Id>> {
        self.many1(|p| p.alt(|p| p.read_io_redirect(), |p| p.read_assignment_word()))
    }

    pub(crate) fn read_cmd_suffix(&mut self) -> PR<Vec<Id>> {
        self.many1(|p| p.alt(|p| p.read_io_redirect(), |p| p.read_cmd_word()))
    }

    pub(crate) fn read_modifier_suffix(&mut self) -> PR<Vec<Id>> {
        self.many1(|p| {
            alt!(
                p,
                |p: &mut Self| p.read_io_redirect(),
                |p: &mut Self| p.read_well_formed_assignment(),
                |p: &mut Self| p.read_cmd_word(),
            )
        })
    }

    pub(crate) fn read_time_suffix(&mut self) -> PR<Vec<Id>> {
        // This fails for quoted variables and such. Fixme?
        let read_flag = |p: &mut Self| {
            p.look_ahead(|p| p.char_('-'))?;
            p.read_cmd_word()
        };
        let mut flags = self.many(read_flag)?;
        let pipeline = self.read_pipeline()?;
        flags.push(pipeline);
        Ok(flags)
    }

    // Fixme: this is a hack that doesn't handle let c='4'"5" or let a\>b
    pub(crate) fn read_let_suffix(&mut self) -> PR<Vec<Id>> {
        self.many1(|p| {
            alt!(
                p,
                |p: &mut Self| p.read_io_redirect(),
                |p: &mut Self| p.try_(|p| p.read_let_expression()),
                |p: &mut Self| p.read_cmd_word(),
            )
        })
    }

    /// `readLetSuffix`'s `readLetExpression`
    fn read_let_expression(&mut self) -> PR<Id> {
        let start_pos = self.get_position();
        let expression = self.read_string_for_parser(|p| p.read_cmd_word())?;
        let (un_quoted, new_pos) = kludge_away_quotes(&expression, start_pos);
        self.sub_parse(
            new_pos,
            |p| {
                let r = p.read_arithmetic_contents()?;
                p.eof()?;
                Ok(r)
            },
            &un_quoted,
        )
    }

    // bash allows a=(b), ksh allows $a=(b). dash allows neither. Let's warn.
    pub(crate) fn read_eval_suffix(&mut self) -> PR<Vec<Id>> {
        self.many1(|p| {
            alt!(p, |p: &mut Self| p.read_io_redirect(), |p: &mut Self| p.read_cmd_word(), |p: &mut Self| {
                // evalFallback
                let pos = p.get_position();
                p.look_ahead(|p| p.char_('('))?;
                p.parse_problem_at(
                    pos,
                    WarningC,
                    1098,
                    "Quote/escape special characters when using eval, e.g. eval \"a=(b)\".",
                );
                p.fail("Unexpected parentheses. Make sure to quote when eval'ing as shell parsers differ.")
            },)
        })
    }

    /// Get whatever a parser would parse as a string
    pub(crate) fn read_string_for_parser<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<String> {
        let pos = self.in_separate_context(|p| {
            p.look_ahead(|p| {
                parser(p)?;
                Ok(p.get_position())
            })
        })?;
        // readUntil pos
        let chars = self.reluctantly_till(
            |p| p.any_char(),
            |p| {
                let here = p.get_position();
                p.guard(here == pos)
            },
        )?;
        Ok(chars.into_iter().collect())
    }

    /// Like readStringForParser, returning the span as a T_Literal
    pub(crate) fn read_literal_for_parser<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<Id> {
        let start = self.start_span();
        let str = self.read_string_for_parser(parser)?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(str)))
    }

    pub(crate) fn read_assignment_word(&mut self) -> PR<Id> {
        self.read_assignment_word_ext(true)
    }

    pub(crate) fn read_well_formed_assignment(&mut self) -> PR<Id> {
        self.read_assignment_word_ext(false)
    }

    pub(crate) fn read_assignment_word_ext(&mut self, lenient: bool) -> PR<Id> {
        self.called("variable assignment", |p| {
            // Parse up to and including the = in a 'try'
            let (id, variable, op, indices) = p.try_(|p| {
                let start = p.start_span();
                let _pos = p.get_position();
                // Check for a leading $ at parse time, to warn for $foo=(bar) which
                // would otherwise cause a parse failure so it can't be checked later.
                let leading_dollar_pos =
                    if lenient { p.option_maybe(|p| p.get_span_positions_for(|p| p.char_('$')))? } else { None };
                let variable = p.read_variable_name()?;
                let indices = p.many(|p| p.read_array_index())?;
                let has_left_space = !p.spacing()?.is_empty();
                let _op_start = p.get_position();
                let id = p.end_span(start);
                let op = p.read_assignment_op()?;
                let _op_end = p.get_position();

                if leading_dollar_pos.is_some() || has_left_space {
                    let has_paren = p.is_followed_by(|p| {
                        p.spacing()?;
                        p.char_('(')
                    })?;
                    if has_paren && let Some((l, r)) = leading_dollar_pos {
                        p.parse_problem_at_with_end(l, r, ErrorC, 1066, "Don't use $ on the left side of assignments.");
                    }

                    // Fail so that this is not parsed as an assignment.
                    return p.fail("");
                }
                // At this point we know for sure.
                Ok((id, variable, op, indices))
            })?;

            let right_pos_start = p.get_position();
            let has_right_space = !p.spacing()?.is_empty();
            let right_pos_end = p.get_position();
            let is_end_of_command = p
                .option_maybe(|p| p.try_(|p| p.look_ahead(|p| p.alt(|p| p.one_of("\r\n;&|)").map(drop), |p| p.eof()))))?
                .is_some();

            if has_right_space || is_end_of_command {
                if variable != "IFS" && has_right_space && !is_end_of_command {
                    p.parse_problem_at_with_end(
                        right_pos_start,
                        right_pos_end,
                        WarningC,
                        1007,
                        "Remove space after = if trying to assign a value (for empty string, use var='' ... ).",
                    );
                }
                let value = p.read_empty_literal()?;
                Ok(p.node(id, Token::T_Assignment(op, variable, indices, value)))
            } else {
                p.optional(|p| {
                    p.look_ahead(|p| p.char_('='))?;
                    p.parse_problem(
                        ErrorC,
                        1097,
                        "Unexpected ==. For assignment, use =. For comparison, use [/[[. Or quote for literal string.",
                    );
                    Ok(())
                })?;

                let value = p.alt(|p| p.read_array(), |p| p.read_normal_word())?;
                p.spacing()?;
                Ok(p.node(id, Token::T_Assignment(op, variable, indices, value)))
            }
        })
    }

    /// `readAssignmentWordExt`'s `readAssignmentOp`
    fn read_assignment_op(&mut self) -> PR<AssignmentMode> {
        // This is probably some kind of ascii art border
        self.unexpecting("===", |p| p.string("==="))?;
        choice!(
            self,
            |p: &mut Self| {
                p.string("+=")?;
                Ok(AssignmentMode::Append)
            },
            |p: &mut Self| {
                p.string("=")?;
                Ok(AssignmentMode::Assign)
            },
        )
    }

    pub(crate) fn read_empty_literal(&mut self) -> PR<Id> {
        let start = self.start_span();
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(String::new())))
    }

    pub(crate) fn read_array_index(&mut self) -> PR<Id> {
        let start = self.start_span();
        self.char_('[')?;
        let pos = self.get_position();
        let str = self.read_string_for_parser(|p| p.read_index_span())?;
        self.char_(']')?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_UnparsedIndex(pos.pos(), str)))
    }

    pub(crate) fn read_array(&mut self) -> PR<Id> {
        self.called("array assignment", |p| {
            let start = p.start_span();
            let opening = p.get_position();
            p.char_('(')?;
            p.optional(|p| {
                p.look_ahead(|p| p.char_('('))?;
                p.parse_problem_at(
                    opening,
                    ErrorC,
                    1116,
                    "Missing $ on a $((..)) expression? (or use ( ( for arrays).",
                );
                Ok(())
            })?;
            p.allspacing()?;
            let words = p.reluctantly_till(|p| p.read_array_element(), |p| p.char_(')'))?;
            p.alt(|p| p.char_(')'), |p| p.fail("Expected ) to close array assignment"))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Array(words)))
        })
    }

    /// `readArray`'s `readElement`
    fn read_array_element(&mut self) -> PR<Id> {
        self.then_skip(|p| p.alt(|p| p.read_array_indexed(), |p| p.read_array_regular()), |p| p.allspacing())
    }

    /// `readArray`'s `readIndexed`
    fn read_array_indexed(&mut self) -> PR<Id> {
        let start = self.start_span();
        let index = self.try_(|p| {
            let x = p.many1(|p| p.read_array_index())?;
            p.char_('=')?;
            Ok(x)
        })?;
        let value = self.alt(
            |p| p.read_array_regular(),
            |p| {
                // nothing
                let start = p.start_span();
                let id = p.end_span(start);
                Ok(p.node(id, Token::T_Literal(String::new())))
            },
        )?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_IndexedElement(index, value)))
    }

    /// `readArray`'s `readRegular`
    fn read_array_regular(&mut self) -> PR<Id> {
        self.alt(|p| p.read_array(), |p| p.read_normal_word())
    }

    /// `tryToken s t`. `t` gets the token's new id: Haskell's `t id`. For a
    /// token constructor it stores the token (`|p, id| p.node(id, Token::T_Semi)`).
    pub(crate) fn try_token<R>(&mut self, s: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        self.try_(|p| {
            let start = p.start_span();
            p.string(s)?;
            let id = p.end_span(start);
            p.spacing()?;
            Ok(t(p, id))
        })
    }

    pub(crate) fn redir_token<R>(&mut self, c: char, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        self.try_(|p| {
            let start = p.start_span();
            p.char_(c)?;
            let id = p.end_span(start);
            p.not_followed_by2(|p| p.char_('('))?;
            Ok(t(p, id))
        })
    }

    pub(crate) fn try_word_token<R>(&mut self, s: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        self.then_skip(|p| p.try_parse_word_token(s, t), |p| p.spacing())
    }

    pub(crate) fn try_parse_word_token<R>(&mut self, keyword: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        self.try_(|p| {
            let pos = p.get_position();
            let start = p.start_span();
            let str = p.anycase_string(keyword)?;
            let id = p.end_span(start);

            p.optional(|p| {
                let c = p.try_(|p| p.look_ahead(|p| p.any_char()))?;
                let msg = format!("You need a space before the {c}.");
                match c {
                    '[' => p.parse_problem(ErrorC, 1069, &msg),
                    '#' => p.parse_problem(ErrorC, 1099, &msg),
                    '!' => p.parse_problem(ErrorC, 1129, &msg),
                    ':' => p.parse_problem(ErrorC, 1130, &msg),
                    _ => {}
                }
                Ok(())
            })?;

            p.look_ahead(|p| p.keyword_separator())?;
            if str != keyword {
                p.parse_problem_at(
                    pos,
                    ErrorC,
                    1081,
                    &format!("Scripts are case sensitive. Use '{keyword}', not '{str}' (or quote if literal)."),
                );
                return p.fail("");
            }
            Ok(t(p, id))
        })
    }
}

/// `readLetSuffix`'s `kludgeAwayQuotes`
fn kludge_away_quotes(s: &str, p: SourcePos) -> (String, SourcePos) {
    let chars: Vec<char> = s.chars().collect();
    match chars.as_slice() {
        [first, middle @ .., last] if "'\"".contains(*first) && first == last => {
            (middle.iter().collect(), update_pos_char(p, *first))
        }
        _ => (s.to_string(), p),
    }
}
