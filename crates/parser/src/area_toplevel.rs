// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 918-1107
// (readCondition, annotations, comments: between the BEGIN/END annotations
// markers) and lines 3128-3673 (keywords and operators, shebang, EOF check,
// rc files, readScript, runParser, parseShell, parseScript, tryWithErrors).
// Some definitions are unused in the Haskell too (g_OPERATOR, readComment, ...).
#![allow(dead_code)]

// toplevel imports
use std::cmp::Ordering;
use std::collections::BTreeMap;

use shellcheck_core::ast::{Annotation, Id};
// `Token` may also be imported by the annotations block; a glob never clashes.
#[allow(unused_imports)]
use shellcheck_core::ast::*;
use shellcheck_core::astlib::{e4m, executable_from_shebang, get_associative_arrays};
use shellcheck_core::interface::Severity::{ErrorC, InfoC};
use shellcheck_core::interface::{
    Code, Comment, ParseSpec, Position, PositionedComment, SystemInterface, new_positioned_comment,
};
use shellcheck_core::prelude::please_report;

use crate::ParseResult;
use crate::engine::{Input, P, PR, ParseError, SourcePos};
use crate::state::{
    Context, Environment, ParseNote, UserState, code_for_parse_note, context_item_disables_code, initial_user_state,
};

// annotations imports
use crate::{alt, choice};
use shellcheck_core::ast::{ConditionType, Token};
use shellcheck_core::data::shell_for_executable;
use shellcheck_core::interface::Severity;

// BEGIN annotations
impl<'a> P<'a> {
    pub(crate) fn read_condition(&mut self) -> PR<Id> {
        self.called("test expression", |p| {
            let opos = p.get_position();
            let start = p.start_span();
            let open = p.alt(|p| p.try_(|p| p.string("[[")), |p| p.string("["))?;
            let single = open == "[";
            let typ = if single { ConditionType::SingleBracket } else { ConditionType::DoubleBracket };

            let pos = p.get_position();
            let space = p.allspacing()?;
            if space.is_empty() {
                p.parse_problem_at_with_end(
                    opos,
                    pos,
                    Severity::ErrorC,
                    1035,
                    &format!(
                        "You need a space after the {}",
                        if single { "[ and before the ]." } else { "[[ and before the ]]." }
                    ),
                );
            }
            if single && space.contains('\n') {
                p.parse_problem_at(pos, Severity::ErrorC, 1080, "You need \\ before line feeds to break lines in [ ].");
            }

            let condition = p.alt(
                |p| p.read_condition_contents(single),
                |p| {
                    p.guard(!space.is_empty())?;
                    p.look_ahead(|p| p.string("]"))?;
                    let id = p.end_span(start);
                    Ok(p.node(id, Token::TC_Empty(typ)))
                },
            )?;

            let cpos = p.get_position();
            let close = alt!(
                p,
                |p| p.try_(|p| p.string("]]")),
                |p| p.string("]"),
                |p| p.fail("Expected test to end here (don't wrap commands in []/[[]])")
            )?;
            let id = p.end_span(start);
            if open == "[[" && close != "]]" {
                p.parse_problem_at(
                    cpos,
                    Severity::ErrorC,
                    1033,
                    "Test expression was opened with double [[ but closed with single ]. Make sure they match.",
                );
            }
            if open == "[" && close != "]" {
                p.parse_problem_at(
                    opos,
                    Severity::ErrorC,
                    1034,
                    "Test expression was opened with single [ but closed with double ]]. Make sure they match.",
                );
            }
            p.spacing()?;
            let close_typ = if close == "]" { ConditionType::SingleBracket } else { ConditionType::DoubleBracket };
            Ok(p.node(id, Token::T_Condition(typ, close_typ, condition)))
        })
    }

    pub(crate) fn read_annotation_prefix(&mut self) -> PR<String> {
        self.char_('#')?;
        self.many(|p| p.linewhitespace())?;
        self.string("shellcheck")
    }

    pub(crate) fn read_annotation(&mut self) -> PR<Vec<Annotation>> {
        self.called("shellcheck directive", |p| {
            p.try_(|p| p.read_annotation_prefix())?;
            p.many1(|p| p.linewhitespace())?;
            p.read_annotation_without_prefix(true)
        })
    }

    pub(crate) fn read_annotation_without_prefix(&mut self, sandboxed: bool) -> PR<Vec<Annotation>> {
        let values = self.many1(|p| p.read_annotation_key(sandboxed))?;
        self.optional(|p| p.read_any_comment())?;
        alt!(
            self,
            |p| p.linefeed().map(drop),
            |p| p.eof(),
            |p| {
                p.parse_note(
                    Severity::ErrorC,
                    1125,
                    "Invalid key=value pair? Ignoring the rest of this directive starting here.",
                );
                p.many(|p| p.none_of("\n"))?;
                alt!(p, |p| p.linefeed().map(drop), |p| p.eof())
            }
        )?;
        self.many(|p| p.linewhitespace())?;
        Ok(values.into_iter().flatten().collect())
    }

    /// `plainOrQuoted p = quoted p <|> p`
    fn annotation_plain_or_quoted<T>(&mut self, f: &dyn Fn(&mut Self) -> PR<T>) -> PR<T> {
        self.alt(|p| p.annotation_quoted(f), |p| f(p))
    }

    fn annotation_quoted<T>(&mut self, f: &dyn Fn(&mut Self) -> PR<T>) -> PR<T> {
        let c = self.one_of("'\"")?;
        let start = self.get_position();
        let excl: String = [c, '\n'].iter().collect();
        let str: String = self.many1(|p| p.none_of(&excl))?.into_iter().collect();
        self.alt(|p| p.char_(c), |p| p.fail("Missing terminating quote for directive."))?;
        self.sub_parse(start, |p| f(p), &str)
    }

    /// `quoted (many1 anyChar) <|> (many1 $ noneOf " \n")`
    fn annotation_filename(&mut self) -> PR<String> {
        self.alt(
            |p| p.annotation_quoted(&|p| Ok(p.many1(|p| p.any_char())?.into_iter().collect::<String>())),
            |p| Ok(p.many1(|p| p.none_of(" \n"))?.into_iter().collect::<String>()),
        )
    }

    fn read_annotation_key(&mut self, sandboxed: bool) -> PR<Vec<Annotation>> {
        let key_pos = self.get_position();
        let key: String = self.many1(|p| p.alt(|p| p.letter(), |p| p.char_('-')))?.into_iter().collect();
        self.alt(|p| p.char_('='), |p| p.fail("Expected '=' after directive key"))?;
        let annotations = match key.as_str() {
            "disable" => self.annotation_plain_or_quoted(&|p| {
                p.sep_by(
                    |p| {
                        // readElement = readRange <|> readAll
                        p.alt(
                            |p| {
                                // readRange
                                let from = p.read_annotation_code()?;
                                let to = choice!(
                                    p,
                                    |p| {
                                        p.char_('-')?;
                                        p.read_annotation_code()
                                    },
                                    |_| Ok(from + 1)
                                )?;
                                Ok(Annotation::DisableComment(from, to))
                            },
                            |p| {
                                // readAll
                                p.string("all")?;
                                Ok(Annotation::DisableComment(0, 1000000))
                            },
                        )
                    },
                    |p| p.char_(','),
                )
            })?,
            "enable" => self.annotation_plain_or_quoted(&|p| {
                p.sep_by(
                    |p| {
                        let name: String =
                            p.many1(|p| p.alt(|p| p.letter(), |p| p.char_('-')))?.into_iter().collect();
                        Ok(Annotation::EnableComment(name))
                    },
                    |p| p.char_(','),
                )
            })?,
            "source" => {
                let filename = self.annotation_filename()?;
                vec![Annotation::SourceOverride(filename)]
            }
            "source-path" => {
                let dirname = self.annotation_filename()?;
                vec![Annotation::SourcePath(dirname)]
            }
            "shell" => {
                let pos = self.get_position();
                let shell = self.annotation_filename()?;
                if shell_for_executable(&shell).is_none() {
                    self.parse_note_at(pos, Severity::ErrorC, 1103, "This shell type is unknown. Use e.g. sh or bash.");
                }
                vec![Annotation::ShellOverride(shell)]
            }
            "extended-analysis" => {
                let pos = self.get_position();
                let value = self.annotation_plain_or_quoted(&|p| {
                    Ok(p.many1(|p| p.letter())?.into_iter().collect::<String>())
                })?;
                match value.as_str() {
                    "true" => vec![Annotation::ExtendedAnalysis(true)],
                    "false" => vec![Annotation::ExtendedAnalysis(false)],
                    _ => {
                        self.parse_note_at(
                            pos,
                            Severity::ErrorC,
                            1146,
                            "Unknown extended-analysis value. Expected true/false.",
                        );
                        vec![]
                    }
                }
            }
            "external-sources" => {
                let pos = self.get_position();
                let value = self.annotation_plain_or_quoted(&|p| {
                    Ok(p.many1(|p| p.letter())?.into_iter().collect::<String>())
                })?;
                match value.as_str() {
                    "true" => {
                        if sandboxed {
                            self.parse_note_at(
                                pos,
                                Severity::ErrorC,
                                1144,
                                "external-sources can only be enabled in .shellcheckrc, not in individual files.",
                            );
                            vec![]
                        } else {
                            vec![Annotation::ExternalSources(true)]
                        }
                    }
                    "false" => vec![Annotation::ExternalSources(false)],
                    _ => {
                        self.parse_note_at(
                            pos,
                            Severity::ErrorC,
                            1145,
                            "Unknown external-sources value. Expected true/false.",
                        );
                        vec![]
                    }
                }
            }
            _ => {
                self.parse_note_at(key_pos, Severity::WarningC, 1107, "This directive is unknown. It will be ignored.");
                self.reluctantly_till(|p| p.any_char(), |p| p.whitespace())?;
                vec![]
            }
        };

        self.many(|p| p.linewhitespace())?;
        Ok(annotations)
    }

    /// `readCode` of the `disable` directive
    fn read_annotation_code(&mut self) -> PR<i64> {
        self.optional(|p| p.string("SC"))?;
        let int: String = self.many1(|p| p.digit())?.into_iter().collect();
        // DIVERGENCE: Haskell reads an arbitrary-size Integer; saturate at i64.
        Ok(int.parse::<i64>().unwrap_or(i64::MAX))
    }

    pub(crate) fn read_annotations(&mut self) -> PR<Vec<Annotation>> {
        let annotations = self.many(|p| p.then_skip(|p| p.read_annotation(), |p| p.allspacing()))?;
        Ok(annotations.into_iter().flatten().collect())
    }

    pub(crate) fn read_comment(&mut self) -> PR<String> {
        self.unexpecting("shellcheck annotation", |p| p.read_annotation_prefix())?;
        self.read_any_comment()
    }

    pub(crate) fn read_any_comment(&mut self) -> PR<String> {
        self.char_('#')?;
        Ok(self.many(|p| p.none_of("\r\n"))?.into_iter().collect())
    }
}
// END annotations

impl<'a> P<'a> {
    pub(crate) fn anycase_string(&mut self, s: &str) -> PR<String> {
        // `toLower`/`toUpper` are simple (single character) case mappings.
        fn simple(mut it: impl Iterator<Item = char>, c: char) -> char {
            match (it.next(), it.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        }
        let mut out = String::new();
        for c in s.chars() {
            let lower = simple(c.to_lowercase(), c);
            let upper = simple(c.to_uppercase(), c);
            out.push(self.alt(|p| p.char_(lower), |p| p.char_(upper))?);
        }
        Ok(out)
    }

    pub(crate) fn g_and_if(&mut self) -> PR<Id> {
        self.try_token("&&", |p, id| p.node(id, Token::T_AND_IF))
    }

    pub(crate) fn g_or_if(&mut self) -> PR<Id> {
        self.try_token("||", |p, id| p.node(id, Token::T_OR_IF))
    }

    pub(crate) fn g_dsemi(&mut self) -> PR<Id> {
        self.try_token(";;", |p, id| p.node(id, Token::T_DSEMI))
    }

    pub(crate) fn g_dless(&mut self) -> PR<Id> {
        self.try_token("<<", |p, id| p.node(id, Token::T_DLESS))
    }

    pub(crate) fn g_dgreat(&mut self) -> PR<Id> {
        self.try_token(">>", |p, id| p.node(id, Token::T_DGREAT))
    }

    pub(crate) fn g_lessand(&mut self) -> PR<Id> {
        self.try_token("<&", |p, id| p.node(id, Token::T_LESSAND))
    }

    pub(crate) fn g_greatand(&mut self) -> PR<Id> {
        self.try_token(">&", |p, id| p.node(id, Token::T_GREATAND))
    }

    pub(crate) fn g_lessgreat(&mut self) -> PR<Id> {
        self.try_token("<>", |p, id| p.node(id, Token::T_LESSGREAT))
    }

    pub(crate) fn g_dlessdash(&mut self) -> PR<Id> {
        self.try_token("<<-", |p, id| p.node(id, Token::T_DLESSDASH))
    }

    pub(crate) fn g_clobber(&mut self) -> PR<Id> {
        self.try_token(">|", |p, id| p.node(id, Token::T_CLOBBER))
    }

    pub(crate) fn g_operator(&mut self) -> PR<Id> {
        crate::alt!(
            self,
            |p| p.g_and_if(),
            |p| p.g_or_if(),
            |p| p.g_dsemi(),
            |p| p.g_dlessdash(),
            |p| p.g_dless(),
            |p| p.g_dgreat(),
            |p| p.g_lessand(),
            |p| p.g_greatand(),
            |p| p.g_lessgreat(),
        )
    }

    pub(crate) fn g_if(&mut self) -> PR<Id> {
        self.try_word_token("if", |p, id| p.node(id, Token::T_If))
    }

    pub(crate) fn g_then(&mut self) -> PR<Id> {
        self.try_word_token("then", |p, id| p.node(id, Token::T_Then))
    }

    pub(crate) fn g_else(&mut self) -> PR<Id> {
        self.try_word_token("else", |p, id| p.node(id, Token::T_Else))
    }

    pub(crate) fn g_elif(&mut self) -> PR<Id> {
        self.try_word_token("elif", |p, id| p.node(id, Token::T_Elif))
    }

    pub(crate) fn g_fi(&mut self) -> PR<Id> {
        self.try_word_token("fi", |p, id| p.node(id, Token::T_Fi))
    }

    pub(crate) fn g_do(&mut self) -> PR<Id> {
        self.try_word_token("do", |p, id| p.node(id, Token::T_Do))
    }

    pub(crate) fn g_done(&mut self) -> PR<Id> {
        self.try_word_token("done", |p, id| p.node(id, Token::T_Done))
    }

    pub(crate) fn g_case(&mut self) -> PR<Id> {
        self.try_word_token("case", |p, id| p.node(id, Token::T_Case))
    }

    pub(crate) fn g_esac(&mut self) -> PR<Id> {
        self.try_word_token("esac", |p, id| p.node(id, Token::T_Esac))
    }

    pub(crate) fn g_while(&mut self) -> PR<Id> {
        self.try_word_token("while", |p, id| p.node(id, Token::T_While))
    }

    pub(crate) fn g_until(&mut self) -> PR<Id> {
        self.try_word_token("until", |p, id| p.node(id, Token::T_Until))
    }

    pub(crate) fn g_for(&mut self) -> PR<Id> {
        self.try_word_token("for", |p, id| p.node(id, Token::T_For))
    }

    pub(crate) fn g_select(&mut self) -> PR<Id> {
        self.try_word_token("select", |p, id| p.node(id, Token::T_Select))
    }

    pub(crate) fn g_in(&mut self) -> PR<Id> {
        let r = self.try_word_token("in", |p, id| p.node(id, Token::T_In))?;
        self.skip_annotation_and_warn()?;
        Ok(r)
    }

    pub(crate) fn g_lbrace(&mut self) -> PR<Id> {
        self.try_word_token("{", |p, id| p.node(id, Token::T_Lbrace))
    }

    /// handled specially due to ksh echo "${ foo; }bar"
    pub(crate) fn g_rbrace(&mut self) -> PR<Id> {
        let start = self.start_span();
        self.char_('}')?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Rbrace))
    }

    pub(crate) fn g_lparen(&mut self) -> PR<Id> {
        self.try_token("(", |p, id| p.node(id, Token::T_Lparen))
    }

    pub(crate) fn g_rparen(&mut self) -> PR<Id> {
        self.try_token(")", |p, id| p.node(id, Token::T_Rparen))
    }

    pub(crate) fn g_bang(&mut self) -> PR<Id> {
        let start = self.start_span();
        self.char_('!')?;
        let id = self.end_span(start);
        self.alt(
            |p| p.spacing1().map(drop),
            |p| {
                let pos = p.get_position();
                p.parse_problem_at(pos, ErrorC, 1035, "You are missing a required space after the !.");
                Ok(())
            },
        )?;
        Ok(self.node(id, Token::T_Bang))
    }

    pub(crate) fn g_semi(&mut self) -> PR<Id> {
        self.not_followed_by2(|p| p.g_dsemi())?;
        self.try_token(";", |p, id| p.node(id, Token::T_Semi))
    }

    pub(crate) fn keyword_separator(&mut self) -> PR<()> {
        crate::alt!(
            self,
            |p| p.eof(),
            |p| p.try_(|p| p.allspacing_or_fail()).map(drop),
            |p| p.one_of(";()[<>&|").map(drop),
        )
    }

    pub(crate) fn read_keyword(&mut self) -> PR<Id> {
        crate::choice!(
            self,
            |p| p.g_then(),
            |p| p.g_else(),
            |p| p.g_elif(),
            |p| p.g_fi(),
            |p| p.g_do(),
            |p| p.g_done(),
            |p| p.g_esac(),
            |p| p.g_rbrace(),
            |p| p.g_rparen(),
            |p| p.g_dsemi(),
        )
    }

    /// `ifParse p t f = (lookAhead (try p) >> t) <|> f`
    pub(crate) fn if_parse<T, U>(
        &mut self,
        p: impl FnOnce(&mut Self) -> PR<T>,
        t: impl FnOnce(&mut Self) -> PR<U>,
        f: impl FnOnce(&mut Self) -> PR<U>,
    ) -> PR<U> {
        self.alt(
            |q| {
                q.look_ahead(|q| q.try_(p))?;
                t(q)
            },
            f,
        )
    }

    pub(crate) fn read_shebang(&mut self) -> PR<Id> {
        fn any_shebang(p: &mut P<'_>) -> PR<()> {
            crate::choice!(
                p,
                |p| p.try_(read_correct),
                |p| p.try_(read_swapped),
                |p| p.try_(read_too_many_spaces),
                |p| p.try_(read_missing_hash),
            )
        }
        fn read_correct(p: &mut P<'_>) -> PR<()> {
            p.string("#!").map(drop)
        }

        fn read_swapped(p: &mut P<'_>) -> PR<()> {
            let start = p.start_span();
            p.string("!#")?;
            let id = p.end_span(start);
            p.parse_problem_at_id(id, ErrorC, 1084, "Use #!, not !#, for the shebang.");
            Ok(())
        }

        fn skip_spaces(p: &mut P<'_>) -> PR<bool> {
            Ok(!p.many(|p| p.linewhitespace())?.is_empty())
        }
        fn read_too_many_spaces(p: &mut P<'_>) -> PR<()> {
            let start_pos = p.get_position();
            let start_spaces = skip_spaces(p)?;
            p.char_('#')?;
            let middle_pos = p.get_position();
            let middle_spaces = skip_spaces(p)?;
            p.char_('!')?;
            if start_spaces {
                p.parse_problem_at(start_pos, ErrorC, 1114, "Remove leading spaces before the shebang.");
            }
            if middle_spaces {
                p.parse_problem_at(middle_pos, ErrorC, 1115, "Remove spaces between # and ! in the shebang.");
            }
            Ok(())
        }

        fn read_missing_hash(p: &mut P<'_>) -> PR<()> {
            let pos = p.get_position();
            p.char_('!')?;
            ensure_path_ahead(p)?;
            p.parse_problem_at(pos, ErrorC, 1104, "Use #!, not just !, for the shebang.");
            Ok(())
        }

        fn read_missing_bang(p: &mut P<'_>) -> PR<()> {
            p.char_('#')?;
            let pos = p.get_position();
            ensure_path_ahead(p)?;
            p.parse_problem_at(pos, ErrorC, 1113, "Use #!, not just #, for the shebang.");
            Ok(())
        }

        fn ensure_path_ahead(p: &mut P<'_>) -> PR<char> {
            p.look_ahead(|p| {
                p.many(|p| p.linewhitespace())?;
                p.char_('/')
            })
        }

        fn with_header(p: &mut P<'_>) -> PR<()> {
            p.try_(|p| {
                p.many1(header_line)?;
                let pos = p.get_position();
                any_shebang(p)?;
                p.parse_problem_at(
                    pos,
                    ErrorC,
                    1128,
                    "The shebang must be on the first line. Delete blanks and move comments.",
                );
                Ok(())
            })
        }

        fn header_line(p: &mut P<'_>) -> PR<char> {
            p.not_followed_by2(any_shebang)?;
            p.many(|p| p.linewhitespace())?;
            p.optional(|p| p.read_any_comment())?;
            p.linefeed()
        }

        let start = self.start_span();
        crate::alt!(self, any_shebang, |p| p.try_(read_missing_bang), with_header)?;
        self.many(|p| p.linewhitespace())?;
        let str: String = self.many(|p| p.none_of("\r\n"))?.into_iter().collect();
        let id = self.end_span(start);
        self.optional(|p| p.carriage_return())?;
        self.optional(|p| p.linefeed())?;
        Ok(self.node(id, Token::T_Literal(str)))
    }

    pub(crate) fn verify_eof(&mut self) -> PR<()> {
        fn if_parsable<T>(p: &mut P<'_>, parser: impl FnOnce(&mut P<'_>) -> PR<T>, code: Code, msg: &str) -> PR<()> {
            p.try_(|p| p.look_ahead(parser))?;
            p.parse_problem(ErrorC, code, msg);
            Ok(())
        }
        self.alt(
            |p| p.eof(),
            |p| {
                crate::choice!(
                    p,
                    |p| if_parsable(p, |p| p.g_lparen(), 1088, "Parsing stopped here. Invalid use of parentheses?"),
                    |p| if_parsable(
                        p,
                        |p| p.read_keyword(),
                        1089,
                        "Parsing stopped here. Is this keyword correctly matched up?"
                    ),
                    |p| {
                        p.parse_problem(ErrorC, 1070, "Parsing stopped here. Mismatched keywords or invalid parentheses?");
                        Ok(())
                    },
                )
            },
        )
    }

    pub(crate) fn read_config_file(&mut self, filename: &str) -> PR<Vec<Annotation>> {
        fn error_for(filename: &str, err: &ParseError) -> String {
            let line = format!("line {}", err.pos.line);
            let suggestion = get_string_from_parsec(&err.messages);
            format!("Failed to process {}, {}: {}", e4m(filename), line, suggestion)
        }

        fn read_config(p: &mut P<'_>, filename: &str, contents: &str) -> PR<Vec<Annotation>> {
            let (result, _) =
                p.run_parser_t(initial_user_state(), filename, Input::new(contents), |p| p.read_config_kvs());
            match result {
                Ok(result) => Ok(result),
                Err(err) => {
                    // Report the error at its location in the config file
                    // (e.g. .shellcheckrc or .editorconfig), not at the
                    // current position in the script being checked.
                    p.parse_problem_at(err.pos, ErrorC, 1134, &error_for(filename, &err));
                    Ok(vec![])
                }
            }
        }

        let should_ignore = self.env.ignore_rc;
        if should_ignore {
            return Ok(vec![]);
        }
        let contents = self.env.system_interface.get_config(filename);
        match contents {
            None => Ok(vec![]),
            Some((file, str)) => read_config(self, &file, &str),
        }
    }

    pub(crate) fn read_config_kvs(&mut self) -> PR<Vec<Annotation>> {
        self.any_spacing_or_comment()?;
        let annotations = self.many(|p| {
            let a = p.read_annotation_without_prefix(false)?;
            p.any_spacing_or_comment()?;
            Ok(a)
        })?;
        self.eof()?;
        Ok(annotations.concat())
    }

    pub(crate) fn any_spacing_or_comment(&mut self) -> PR<()> {
        self.many(|p| p.alt(|p| p.allspacing_or_fail().map(drop), |p| p.read_any_comment().map(drop)))
            .map(drop)
    }

    pub(crate) fn read_script_file(&mut self, sourced: bool) -> PR<Id> {
        fn is_valid_shell(s: &str) -> Option<bool> {
            let good_shells = ["sh", "ash", "dash", "busybox sh", "bash", "bats", "ksh", "oksh"];
            let bad_shells =
                ["awk", "csh", "expect", "fish", "perl", "python", "python3", "ruby", "tcsh", "zsh"];
            let good = s.is_empty() || good_shells.iter().any(|g| s.starts_with(g));
            let bad = bad_shells.iter().any(|b| s.starts_with(b));
            if good {
                Some(true)
            } else if bad {
                Some(false)
            } else {
                None
            }
        }

        fn verify_shebang(p: &mut P<'_>, pos: SourcePos, s: &str) {
            match is_valid_shell(s) {
                Some(true) => {}
                Some(false) => p.parse_problem_at(
                    pos,
                    ErrorC,
                    1071,
                    "ShellCheck only supports sh/bash/dash/ksh/'busybox sh' scripts. Sorry!",
                ),
                None => p.parse_problem_at(
                    pos,
                    ErrorC,
                    1008,
                    "This shebang was unrecognized. ShellCheck only supports sh/bash/dash/ksh/'busybox sh'. Add a 'shell' directive to specify.",
                ),
            }
        }

        fn read_utf8_bom(p: &mut P<'_>) -> PR<String> {
            p.called("Byte Order Mark", |p| p.string("\u{FEFF}"))
        }

        let start = self.start_span();
        let pos = self.get_position();
        let rc_annotations = if sourced {
            vec![]
        } else {
            let filename = self.env.current_filename.clone();
            self.read_config_file(&filename)?
        };

        // Put the rc annotations on the stack so that one can ignore e.g. SC1084 in .shellcheckrc
        self.with_annotations(rc_annotations.clone(), move |p| {
            let has_bom = p.was_included(read_utf8_bom)?;
            let shebang = p.alt(|p| p.read_shebang(), |p| p.read_empty_literal())?;
            let shebang_string = match p.user.ast.get(shebang) {
                Token::T_Literal(s) => s.clone(),
                _ => panic!("{}", please_report("shebang is not a T_Literal")),
            };
            p.allspacing()?;
            let annotation_start = p.start_span();
            let file_annotations = p.read_annotations()?;

            // Similarly put the filewide annotations on the stack to allow earlier suppression
            p.with_annotations(file_annotations.clone(), move |p| {
                if has_bom {
                    p.parse_problem_at(
                        pos,
                        ErrorC,
                        1082,
                        "This file has a UTF-8 BOM. Remove it with: LC_CTYPE=C sed '1s/^...//' < yourscript .",
                    );
                }
                let mut annotations = file_annotations;
                annotations.extend(rc_annotations);
                let annotation_id = p.end_span(annotation_start);
                let shell_annotation_specified =
                    annotations.iter().any(|x| matches!(x, Annotation::ShellOverride(_)));
                let shell_flag_specified = p.env.shell_type_override.is_some();
                let ignore_shebang = shell_annotation_specified || shell_flag_specified;

                let executable = executable_from_shebang(&shebang_string);
                if !ignore_shebang {
                    verify_shebang(p, pos, &executable);
                }
                if ignore_shebang || is_valid_shell(&executable) != Some(false) {
                    let commands = p.read_compound_list_or_empty()?;
                    let id = p.end_span(start);
                    p.read_pending_here_docs()?;
                    p.verify_eof()?;
                    p.node(id, Token::T_Script(shebang, commands));
                    let script = p.node(annotation_id, Token::T_Annotation(annotations, id));
                    let map = p.user.here_doc_map();
                    let root = p.reattach_here_docs(script, &map);
                    p.reparse_indices(root)
                } else {
                    p.many(|p| p.any_char())?;
                    let id = p.end_span(start);
                    Ok(p.node(id, Token::T_Script(shebang, vec![])))
                }
            })
        })
    }

    pub(crate) fn read_script(&mut self) -> PR<Id> {
        self.read_script_file(false)
    }

    // debugParse, debugParseScript: ghci helpers, not ported.
    // testEnvironment, isOk, isWarning, isNotOk, getParseOutput, parsesCleanly:
    // see `crate::testing`.

    /// `parseWithNotes`. DIVERGENCE: moves the final `UserState` out of the
    /// parser instead of copying it (it is only used as the last step).
    pub(crate) fn parse_with_notes<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<(T, UserState)> {
        let item = parser(self)?;
        let state = std::mem::take(&mut self.user);
        Ok((item, state))
    }

    /// Needs `&self` to compare file names (see `P::compare_pos`).
    pub(crate) fn compare_notes(&self, a: &ParseNote, b: &ParseNote) -> Ordering {
        self.compare_pos(a.start, b.start)
            .then_with(|| self.compare_pos(a.end, b.end))
            .then_with(|| a.severity.cmp(&b.severity))
    }

    pub(crate) fn sort_notes(&self, notes: Vec<ParseNote>) -> Vec<ParseNote> {
        let mut notes = notes;
        notes.sort_by(|a, b| self.compare_notes(a, b));
        notes
    }
}

pub(crate) fn make_error_for(parsec_error: &ParseError) -> ParseNote {
    let pos = parsec_error.pos;
    ParseNote::new(pos, pos, ErrorC, 1072, &get_string_from_parsec(&parsec_error.messages))
}

/// `errors` are the `Message` texts of the Parsec error (`ParseError::messages`).
/// UnExpect, SysUnExpect and Expect messages are not kept (see
/// `engine::ParseError`): due to not knowing Parsec, none of these are
/// actually helpful. <?> has been hidden and we only show explicit fail
/// statements.
pub(crate) fn get_string_from_parsec(errors: &[String]) -> String {
    let first = errors.iter().rev().find(|s| !s.is_empty()).map(|s| format!("{s}.")).unwrap_or_default();
    first + " Fix any mentioned problems and try again."
}

/// `runParser`. DIVERGENCE: returns the whole final parser instead of only
/// its `SystemState` (`P::sys`), because turning notes into
/// `PositionedComment`s needs the parser's file-name table.
pub(crate) fn run_parser<'a, T>(
    env: Environment<'a>,
    p: impl FnOnce(&mut P<'a>) -> PR<T>,
    filename: &str,
    contents: &str,
) -> (Result<T, ParseError>, P<'a>) {
    let mut parser = P::new(env, filename, contents);
    let r = parser.run(p);
    (r, parser)
}

// system = lift . lift . lift: not needed; `P::env.system_interface` is called directly.

pub(crate) fn parse_shell(env: Environment<'_>, name: &str, contents: &str) -> ParseResult {
    // `nub`: keep the first occurrence of each note.
    fn nub(list: Vec<ParseNote>) -> Vec<ParseNote> {
        let mut out: Vec<ParseNote> = Vec::new();
        for n in list {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
    // A final pass for ignoring parse errors after failed parsing
    fn is_ignored(stack: &[Context], note: &ParseNote) -> bool {
        stack.iter().any(|c| context_item_disables_code(false, code_for_parse_note(note), c))
    }

    let (result, p) = run_parser(env, |p| p.parse_with_notes(|p| p.read_script()), name, contents);
    // The Haskell note lists are newest first; ours are oldest first.
    let parse_problems: Vec<ParseNote> = p.sys.parse_problems.iter().rev().cloned().collect();
    match result {
        Ok((script, userstate)) => {
            let mut notes: Vec<ParseNote> = userstate.parse_notes.iter().rev().cloned().collect();
            notes.extend(parse_problems);
            ParseResult {
                comments: nub(notes).iter().map(|n| p.to_positioned_comment(n)).collect(),
                ast: userstate.ast,
                root: Some(script),
            }
        }
        Err(err) => {
            let context = &p.sys.context_stack;
            let mut notes = notes_for_context(context);
            notes.push(make_error_for(&err));
            let mut notes: Vec<ParseNote> = notes.into_iter().filter(|n| !is_ignored(context, n)).collect();
            notes.extend(parse_problems);
            ParseResult {
                comments: notes.iter().map(|n| p.to_positioned_comment(n)).collect(),
                ast: Default::default(),
                root: None,
            }
        }
    }
}

pub(crate) fn notes_for_context(list: &[Context]) -> Vec<ParseNote> {
    let first = |(pos, str): (SourcePos, &str)| {
        ParseNote::new(pos, pos, ErrorC, 1073, &format!("Couldn't parse this {str}. Fix to allow more checks."))
    };
    let second = |(pos, str): (SourcePos, &str)| {
        ParseNote::new(pos, pos, InfoC, 1009, &format!("The mentioned syntax error was in this {str}."))
    };
    let names: Vec<(SourcePos, &str)> = list
        .iter()
        .filter_map(|c| match c {
            Context::ContextName(pos, str) => Some((*pos, str.as_str())),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    if let Some(&x) = names.first() {
        out.push(first(x));
    }
    if let Some(&x) = names.get(1) {
        out.push(second(x));
    }
    out
}

impl<'a> P<'a> {
    /// Go over all T_UnparsedIndex and reparse them as either arithmetic or text
    /// depending on declare -A statements.
    // DIVERGENCE: the Haskell rebuilds the tree with `analyze blank blank f`;
    // here `f` runs on each token in the same (post-) order and rewrites it in
    // place in the arena, so ids are allocated at the same points.
    pub(crate) fn reparse_indices(&mut self, root: Id) -> PR<Id> {
        let associative = get_associative_arrays(&self.user.ast, root);
        self.reparse_process(&associative, root)
    }

    fn reparse_process(&mut self, associative: &[String], root: Id) -> PR<Id> {
        let mut order = Vec::new();
        self.user.ast.analyze(root, &mut |_| {}, &mut |id| order.push(id));
        for id in order {
            self.reparse_f(associative, id)?;
        }
        Ok(root)
    }

    fn reparse_f(&mut self, associative: &[String], id: Id) -> PR<()> {
        match self.user.ast.get(id).clone() {
            Token::T_Assignment(mode, name, indices, value) => {
                let mut new_indices = Vec::with_capacity(indices.len());
                for i in indices {
                    new_indices.push(self.fix_assignment_index(associative, &name, i)?);
                }
                if let Token::T_Array(words) = self.user.ast.get(value).clone() {
                    let mut new_words = Vec::with_capacity(words.len());
                    for w in words {
                        new_words.push(self.fix_index_element(associative, &name, w)?);
                    }
                    self.user.ast.set(value, Token::T_Array(new_words));
                }
                self.user.ast.set(id, Token::T_Assignment(mode, name, new_indices, value));
            }
            Token::TA_Variable(name, indices) => {
                let mut new_indices = Vec::with_capacity(indices.len());
                for i in indices {
                    new_indices.push(self.fix_assignment_index(associative, &name, i)?);
                }
                self.user.ast.set(id, Token::TA_Variable(name, new_indices));
            }
            _ => {}
        }
        Ok(())
    }

    fn fix_index_element(&mut self, associative: &[String], name: &str, word: Id) -> PR<Id> {
        match self.user.ast.get(word).clone() {
            Token::T_IndexedElement(indices, value) => {
                let mut new = Vec::with_capacity(indices.len());
                for i in indices {
                    new.push(self.fix_assignment_index(associative, name, i)?);
                }
                Ok(self.node(word, Token::T_IndexedElement(new, value)))
            }
            _ => Ok(word),
        }
    }

    fn fix_assignment_index(&mut self, associative: &[String], name: &str, word: Id) -> PR<Id> {
        match self.user.ast.get(word).clone() {
            Token::T_UnparsedIndex(pos, src) => {
                // DIVERGENCE: the arena `Pos` has no file name; it is the
                // file of the T_UnparsedIndex token's own span.
                let (span_start, _) = self.get_span_for_id(word);
                let pos = SourcePos { file: span_start.file, line: pos.line, column: pos.column };
                let idx = self.parsed(associative, name, pos, &src)?;
                self.reparse_process(associative, idx) // Recursively parse for cases like x[y[z=1]]=1
            }
            _ => Ok(word),
        }
    }

    fn parsed(&mut self, associative: &[String], name: &str, pos: SourcePos, src: &str) -> PR<Id> {
        if associative.iter().any(|s| s == name) {
            self.sub_parse(pos, |p| p.called("associative array index", |p| p.read_index_span()), src)
        } else {
            self.sub_parse(
                pos,
                |p| {
                    p.called("arithmetic array index expression", |p| {
                        p.optional(|p| p.space())?;
                        p.read_arithmetic_contents()
                    })
                },
                src,
            )
        }
    }

    /// `map` is `UserState::here_doc_map()`.
    // DIVERGENCE: `doTransform` rebuilds the tree; here the T_HereDoc tokens
    // are rewritten in place.
    pub(crate) fn reattach_here_docs(&mut self, root: Id, map: &BTreeMap<Id, Vec<Id>>) -> Id {
        let mut here_docs = Vec::new();
        let ast = &self.user.ast;
        ast.analyze(
            root,
            &mut |_| {},
            &mut |id| {
                if let Token::T_HereDoc(_, _, _, list) = ast.get(id) {
                    if list.is_empty() {
                        here_docs.push(id);
                    }
                }
            },
        );
        for id in here_docs {
            if let Some(list) = map.get(&id) {
                if let Token::T_HereDoc(dash, quote, string, _) = self.user.ast.get(id).clone() {
                    self.user.ast.set(id, Token::T_HereDoc(dash, quote, string, list.clone()));
                }
            }
        }
        root
    }

    /// Needs `&self` to look up the file name.
    pub(crate) fn to_positioned_comment(&self, note: &ParseNote) -> PositionedComment {
        PositionedComment {
            start_pos: self.pos_to_pos(note.start),
            end_pos: self.pos_to_pos(note.end),
            comment: Comment { severity: note.severity, code: note.code, message: note.message.clone() },
            ..new_positioned_comment()
        }
    }

    pub(crate) fn pos_to_pos(&self, sp: SourcePos) -> Position {
        Position { file: self.source_name(sp).to_string(), line: sp.line as i64, column: sp.column as i64 }
    }

    pub(crate) fn start_end_pos_to_pos(&self, se: (SourcePos, SourcePos)) -> (Position, Position) {
        (self.pos_to_pos(se.0), self.pos_to_pos(se.1))
    }
}

// TODO: Clean up crusty old code that this is layered on top of
pub(crate) fn parse_script(sys: &dyn SystemInterface, spec: &ParseSpec) -> ParseResult {
    let env = Environment {
        system_interface: sys,
        check_sourced: spec.check_sourced,
        current_filename: spec.filename.clone(),
        ignore_rc: spec.ignore_rc,
        shell_type_override: spec.shell_type_override,
    };
    parse_shell(env, &spec.filename, &spec.script)
}

impl<'a> P<'a> {
    /// Same as 'try' but emit syntax errors if the parse fails.
    // DIVERGENCE: instead of copying the user state into the separate parse,
    // it is moved there and, on failure, restored with a checkpoint snapshot
    // taken first (the inner parse only appends to it).
    pub(crate) fn try_with_errors<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let snapshot = self.user.snapshot();
        let old_context = self.get_current_contexts();
        let input = self.get_input();
        let pos = self.get_position();
        let name = self.source_name(pos).to_string();
        let userstate = std::mem::take(&mut self.user);
        let (result, mut end_state) = self.run_parser_t(userstate, &name, input, |p| {
            p.set_position(pos);
            let result = parser(p)?;
            let end_pos = p.get_position();
            let end_input = p.get_input();
            Ok((result, end_pos, end_input))
        });
        match result {
            Ok((result, end_pos, end_input)) => {
                // The outer state is the end state from here on: putState
                // comes right after, and `eof` cannot backtrack past it.
                self.user = end_state;
                // 'many' objects if we don't consume anything at all, so read a dummy value
                self.alt(|p| p.any_char().map(drop), |p| p.eof())?;
                self.set_position(end_pos);
                self.set_input(end_input);
                Ok(result)
            }
            Err(err) => {
                end_state.restore(&snapshot);
                self.user = end_state;
                let new_context = self.get_current_contexts();
                self.add_parse_problem(make_error_for(&err));
                for n in notes_for_context(&new_context) {
                    self.add_parse_problem(n);
                }
                self.set_current_contexts(old_context);
                self.fail("")
            }
        }
    }
}
