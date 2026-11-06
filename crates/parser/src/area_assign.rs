// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2909-3127:
// command prefixes/suffixes (let/eval/time/declare), assignments, arrays,
// token helpers.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_cmd_prefix(&mut self) -> PR<Vec<Id>> {
        unported!("readCmdPrefix")
    }

    pub(crate) fn read_cmd_suffix(&mut self) -> PR<Vec<Id>> {
        unported!("readCmdSuffix")
    }

    pub(crate) fn read_modifier_suffix(&mut self) -> PR<Vec<Id>> {
        unported!("readModifierSuffix")
    }

    pub(crate) fn read_time_suffix(&mut self) -> PR<Vec<Id>> {
        unported!("readTimeSuffix")
    }

    pub(crate) fn read_let_suffix(&mut self) -> PR<Vec<Id>> {
        unported!("readLetSuffix")
    }

    pub(crate) fn read_eval_suffix(&mut self) -> PR<Vec<Id>> {
        unported!("readEvalSuffix")
    }

    /// Get whatever a parser would parse as a string
    pub(crate) fn read_string_for_parser<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<String> {
        unported!("readStringForParser")
    }

    /// Like readStringForParser, returning the span as a T_Literal
    pub(crate) fn read_literal_for_parser<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<Id> {
        unported!("readLiteralForParser")
    }

    pub(crate) fn read_assignment_word(&mut self) -> PR<Id> {
        unported!("readAssignmentWord")
    }

    pub(crate) fn read_well_formed_assignment(&mut self) -> PR<Id> {
        unported!("readWellFormedAssignment")
    }

    pub(crate) fn read_assignment_word_ext(&mut self, lenient: bool) -> PR<Id> {
        unported!("readAssignmentWordExt")
    }

    pub(crate) fn read_empty_literal(&mut self) -> PR<Id> {
        unported!("readEmptyLiteral")
    }

    pub(crate) fn read_array_index(&mut self) -> PR<Id> {
        unported!("readArrayIndex")
    }

    pub(crate) fn read_array(&mut self) -> PR<Id> {
        unported!("readArray")
    }

    /// `tryToken s t`. `t` gets the token's new id: Haskell's `t id`. For a
    /// token constructor it stores the token (`|p, id| p.node(id, Token::T_Semi)`).
    pub(crate) fn try_token<R>(&mut self, s: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        unported!("tryToken")
    }

    pub(crate) fn redir_token<R>(&mut self, c: char, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        unported!("redirToken")
    }

    pub(crate) fn try_word_token<R>(&mut self, s: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        unported!("tryWordToken")
    }

    pub(crate) fn try_parse_word_token<R>(&mut self, keyword: &str, t: impl FnOnce(&mut Self, Id) -> R) -> PR<R> {
        unported!("tryParseWordToken")
    }
}
