// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 918-1107
// (readCondition, annotations, comments: between the BEGIN/END annotations
// markers) and lines 3128-3673 (keywords and operators, shebang, EOF check,
// rc files, readScript, runParser, parseShell, parseScript, tryWithErrors).
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use std::cmp::Ordering;
use std::collections::BTreeMap;

use shellcheck_core::ast::{Annotation, Id};
use shellcheck_core::interface::{ParseSpec, Position, PositionedComment, SystemInterface};

use crate::ParseResult;
use crate::engine::{P, PR, ParseError, SourcePos};
use crate::state::{Context, Environment, ParseNote, SystemState, UserState};
use crate::unported;

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
        unported!("anycaseString")
    }

    pub(crate) fn g_and_if(&mut self) -> PR<Id> {
        unported!("g_AND_IF")
    }

    pub(crate) fn g_or_if(&mut self) -> PR<Id> {
        unported!("g_OR_IF")
    }

    pub(crate) fn g_dsemi(&mut self) -> PR<Id> {
        unported!("g_DSEMI")
    }

    pub(crate) fn g_dless(&mut self) -> PR<Id> {
        unported!("g_DLESS")
    }

    pub(crate) fn g_dgreat(&mut self) -> PR<Id> {
        unported!("g_DGREAT")
    }

    pub(crate) fn g_lessand(&mut self) -> PR<Id> {
        unported!("g_LESSAND")
    }

    pub(crate) fn g_greatand(&mut self) -> PR<Id> {
        unported!("g_GREATAND")
    }

    pub(crate) fn g_lessgreat(&mut self) -> PR<Id> {
        unported!("g_LESSGREAT")
    }

    pub(crate) fn g_dlessdash(&mut self) -> PR<Id> {
        unported!("g_DLESSDASH")
    }

    pub(crate) fn g_clobber(&mut self) -> PR<Id> {
        unported!("g_CLOBBER")
    }

    pub(crate) fn g_operator(&mut self) -> PR<Id> {
        unported!("g_OPERATOR")
    }

    pub(crate) fn g_if(&mut self) -> PR<Id> {
        unported!("g_If")
    }

    pub(crate) fn g_then(&mut self) -> PR<Id> {
        unported!("g_Then")
    }

    pub(crate) fn g_else(&mut self) -> PR<Id> {
        unported!("g_Else")
    }

    pub(crate) fn g_elif(&mut self) -> PR<Id> {
        unported!("g_Elif")
    }

    pub(crate) fn g_fi(&mut self) -> PR<Id> {
        unported!("g_Fi")
    }

    pub(crate) fn g_do(&mut self) -> PR<Id> {
        unported!("g_Do")
    }

    pub(crate) fn g_done(&mut self) -> PR<Id> {
        unported!("g_Done")
    }

    pub(crate) fn g_case(&mut self) -> PR<Id> {
        unported!("g_Case")
    }

    pub(crate) fn g_esac(&mut self) -> PR<Id> {
        unported!("g_Esac")
    }

    pub(crate) fn g_while(&mut self) -> PR<Id> {
        unported!("g_While")
    }

    pub(crate) fn g_until(&mut self) -> PR<Id> {
        unported!("g_Until")
    }

    pub(crate) fn g_for(&mut self) -> PR<Id> {
        unported!("g_For")
    }

    pub(crate) fn g_select(&mut self) -> PR<Id> {
        unported!("g_Select")
    }

    pub(crate) fn g_in(&mut self) -> PR<Id> {
        unported!("g_In")
    }

    pub(crate) fn g_lbrace(&mut self) -> PR<Id> {
        unported!("g_Lbrace")
    }

    /// handled specially due to ksh echo "${ foo; }bar"
    pub(crate) fn g_rbrace(&mut self) -> PR<Id> {
        unported!("g_Rbrace")
    }

    pub(crate) fn g_lparen(&mut self) -> PR<Id> {
        unported!("g_Lparen")
    }

    pub(crate) fn g_rparen(&mut self) -> PR<Id> {
        unported!("g_Rparen")
    }

    pub(crate) fn g_bang(&mut self) -> PR<Id> {
        unported!("g_Bang")
    }

    pub(crate) fn g_semi(&mut self) -> PR<Id> {
        unported!("g_Semi")
    }

    pub(crate) fn keyword_separator(&mut self) -> PR<()> {
        unported!("keywordSeparator")
    }

    pub(crate) fn read_keyword(&mut self) -> PR<Id> {
        unported!("readKeyword")
    }

    /// `ifParse p t f = (lookAhead (try p) >> t) <|> f`
    pub(crate) fn if_parse<T, U>(
        &mut self,
        p: impl FnOnce(&mut Self) -> PR<T>,
        t: impl FnOnce(&mut Self) -> PR<U>,
        f: impl FnOnce(&mut Self) -> PR<U>,
    ) -> PR<U> {
        unported!("ifParse")
    }

    pub(crate) fn read_shebang(&mut self) -> PR<Id> {
        unported!("readShebang")
    }

    pub(crate) fn verify_eof(&mut self) -> PR<()> {
        unported!("verifyEof")
    }

    pub(crate) fn read_config_file(&mut self, filename: &str) -> PR<Vec<Annotation>> {
        unported!("readConfigFile")
    }

    pub(crate) fn read_config_kvs(&mut self) -> PR<Vec<Annotation>> {
        unported!("readConfigKVs")
    }

    pub(crate) fn any_spacing_or_comment(&mut self) -> PR<()> {
        unported!("anySpacingOrComment")
    }

    pub(crate) fn read_script_file(&mut self, sourced: bool) -> PR<Id> {
        unported!("readScriptFile")
    }

    pub(crate) fn read_script(&mut self) -> PR<Id> {
        unported!("readScript")
    }

    // debugParse, debugParseScript: ghci helpers, not ported.
    // testEnvironment, isOk, isWarning, isNotOk, getParseOutput, parsesCleanly:
    // see `crate::testing`.

    /// `parseWithNotes`. DIVERGENCE: moves the final `UserState` out of the
    /// parser instead of copying it (it is only used as the last step).
    pub(crate) fn parse_with_notes<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<(T, UserState)> {
        unported!("parseWithNotes")
    }

    /// Needs `&self` to compare file names (see `P::compare_pos`).
    pub(crate) fn compare_notes(&self, a: &ParseNote, b: &ParseNote) -> Ordering {
        unported!("compareNotes")
    }

    pub(crate) fn sort_notes(&self, notes: Vec<ParseNote>) -> Vec<ParseNote> {
        unported!("sortNotes")
    }
}

pub(crate) fn make_error_for(parsec_error: &ParseError) -> ParseNote {
    unported!("makeErrorFor")
}

/// `errors` are the `Message` texts of the Parsec error (`ParseError::messages`).
pub(crate) fn get_string_from_parsec(errors: &[String]) -> String {
    unported!("getStringFromParsec")
}

pub(crate) fn run_parser<'a, T>(
    env: Environment<'a>,
    p: impl FnOnce(&mut P<'a>) -> PR<T>,
    filename: &str,
    contents: &str,
) -> (Result<T, ParseError>, SystemState) {
    unported!("runParser")
}

// system = lift . lift . lift: not needed; `P::env.system_interface` is called directly.

pub(crate) fn parse_shell(env: Environment<'_>, name: &str, contents: &str) -> ParseResult {
    unported!("parseShell")
}

pub(crate) fn notes_for_context(list: &[Context]) -> Vec<ParseNote> {
    unported!("notesForContext")
}

impl<'a> P<'a> {
    /// Go over all T_UnparsedIndex and reparse them as either arithmetic or text
    /// depending on declare -A statements.
    pub(crate) fn reparse_indices(&mut self, root: Id) -> PR<Id> {
        unported!("reparseIndices")
    }

    /// `map` is `UserState::here_doc_map()`.
    pub(crate) fn reattach_here_docs(&mut self, root: Id, map: &BTreeMap<Id, Vec<Id>>) -> Id {
        unported!("reattachHereDocs")
    }

    /// Needs `&self` to look up the file name.
    pub(crate) fn to_positioned_comment(&self, note: &ParseNote) -> PositionedComment {
        unported!("toPositionedComment")
    }

    pub(crate) fn pos_to_pos(&self, sp: SourcePos) -> Position {
        unported!("posToPos")
    }

    pub(crate) fn start_end_pos_to_pos(&self, se: (SourcePos, SourcePos)) -> (Position, Position) {
        unported!("startEndPosToPos")
    }
}

pub(crate) fn parse_script(sys: &dyn SystemInterface, spec: &ParseSpec) -> ParseResult {
    unported!("parseScript")
}

impl<'a> P<'a> {
    /// Same as 'try' but emit syntax errors if the parse fails.
    pub(crate) fn try_with_errors<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        unported!("tryWithErrors")
    }
}
