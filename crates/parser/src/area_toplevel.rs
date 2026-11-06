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

// BEGIN annotations
impl<'a> P<'a> {
    pub(crate) fn read_condition(&mut self) -> PR<Id> {
        unported!("readCondition")
    }

    pub(crate) fn read_annotation_prefix(&mut self) -> PR<String> {
        unported!("readAnnotationPrefix")
    }

    pub(crate) fn read_annotation(&mut self) -> PR<Vec<Annotation>> {
        unported!("readAnnotation")
    }

    pub(crate) fn read_annotation_without_prefix(&mut self, sandboxed: bool) -> PR<Vec<Annotation>> {
        unported!("readAnnotationWithoutPrefix")
    }

    pub(crate) fn read_annotations(&mut self) -> PR<Vec<Annotation>> {
        unported!("readAnnotations")
    }

    pub(crate) fn read_comment(&mut self) -> PR<String> {
        unported!("readComment")
    }

    pub(crate) fn read_any_comment(&mut self) -> PR<String> {
        unported!("readAnyComment")
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
