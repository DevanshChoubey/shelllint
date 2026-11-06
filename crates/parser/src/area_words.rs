// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1108-1611:
// words, quotes, backticks, globs, extglobs, escapes, brace expansions.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR, SourcePos};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_normal_word(&mut self) -> PR<Id> {
        unported!("readNormalWord")
    }

    pub(crate) fn read_pattern_word(&mut self) -> PR<Id> {
        unported!("readPatternWord")
    }

    pub(crate) fn read_normalish_word(&mut self, end: &str, terms: &[&str]) -> PR<Id> {
        unported!("readNormalishWord")
    }

    pub(crate) fn read_index_span(&mut self) -> PR<Id> {
        unported!("readIndexSpan")
    }

    pub(crate) fn check_possible_termination(&mut self, pos: SourcePos, x: &[Id], terminators: &[&str]) {
        unported!("checkPossibleTermination")
    }

    pub(crate) fn read_normal_word_part(&mut self, end: &str) -> PR<Id> {
        unported!("readNormalWordPart")
    }

    pub(crate) fn read_space_part(&mut self) -> PR<Id> {
        unported!("readSpacePart")
    }

    pub(crate) fn read_dollar_braced_word(&mut self) -> PR<Id> {
        unported!("readDollarBracedWord")
    }

    pub(crate) fn read_dollar_braced_part(&mut self) -> PR<Id> {
        unported!("readDollarBracedPart")
    }

    pub(crate) fn read_dollar_braced_literal(&mut self) -> PR<Id> {
        unported!("readDollarBracedLiteral")
    }

    pub(crate) fn read_param_sub_special_char(&mut self) -> PR<Id> {
        unported!("readParamSubSpecialChar")
    }

    pub(crate) fn read_proc_sub(&mut self) -> PR<Id> {
        unported!("readProcSub")
    }

    pub(crate) fn read_single_quoted(&mut self) -> PR<Id> {
        unported!("readSingleQuoted")
    }

    pub(crate) fn read_single_quoted_literal(&mut self) -> PR<String> {
        unported!("readSingleQuotedLiteral")
    }

    pub(crate) fn read_single_quoted_part(&mut self) -> PR<String> {
        unported!("readSingleQuotedPart")
    }

    pub(crate) fn read_quoted_back_ticked(&mut self) -> PR<Id> {
        unported!("readQuotedBackTicked")
    }

    pub(crate) fn read_unquoted_back_ticked(&mut self) -> PR<Id> {
        unported!("readUnquotedBackTicked")
    }

    pub(crate) fn read_back_ticked(&mut self, quoted: bool) -> PR<Id> {
        unported!("readBackTicked")
    }

    /// Run a parser on a new input, such as for `..` or here documents.
    pub(crate) fn sub_parse<T>(&mut self, pos: SourcePos, parser: impl FnOnce(&mut Self) -> PR<T>, input: &str) -> PR<T> {
        unported!("subParse")
    }

    /// Parse something, but forget all parseProblems
    pub(crate) fn in_separate_context<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        unported!("inSeparateContext")
    }

    /// Parse something, but forget all parseProblems on failure
    pub(crate) fn forget_on_failure<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        unported!("forgetOnFailure")
    }

    pub(crate) fn parse_forgetting_context<T>(
        &mut self,
        also_on_success: bool,
        parser: impl FnOnce(&mut Self) -> PR<T>,
    ) -> PR<T> {
        unported!("parseForgettingContext")
    }

    pub(crate) fn read_double_quoted(&mut self) -> PR<Id> {
        unported!("readDoubleQuoted")
    }

    pub(crate) fn suggest_forgot_closing_quote(&mut self, start_pos: SourcePos, end_pos: SourcePos, name: &str) {
        unported!("suggestForgotClosingQuote")
    }

    pub(crate) fn double_quoted_part(&mut self) -> PR<Id> {
        unported!("doubleQuotedPart")
    }

    pub(crate) fn read_double_literal(&mut self) -> PR<Id> {
        unported!("readDoubleLiteral")
    }

    pub(crate) fn read_double_literal_part(&mut self) -> PR<String> {
        unported!("readDoubleLiteralPart")
    }

    pub(crate) fn read_normal_literal(&mut self, end: &str) -> PR<Id> {
        unported!("readNormalLiteral")
    }

    pub(crate) fn read_glob(&mut self) -> PR<Id> {
        unported!("readGlob")
    }

    pub(crate) fn read_normal_literal_part(&mut self, custom_end: &str) -> PR<String> {
        unported!("readNormalLiteralPart")
    }

    pub(crate) fn read_normal_escaped(&mut self) -> PR<String> {
        unported!("readNormalEscaped")
    }

    pub(crate) fn read_extglob(&mut self) -> PR<Id> {
        unported!("readExtglob")
    }

    pub(crate) fn read_extglob_part(&mut self) -> PR<Id> {
        unported!("readExtglobPart")
    }

    pub(crate) fn read_single_escaped(&mut self) -> PR<String> {
        unported!("readSingleEscaped")
    }

    pub(crate) fn read_double_escaped(&mut self) -> PR<String> {
        unported!("readDoubleEscaped")
    }

    pub(crate) fn read_brace_escaped(&mut self) -> PR<String> {
        unported!("readBraceEscaped")
    }

    pub(crate) fn read_generic_literal(&mut self, end_chars: &str) -> PR<String> {
        unported!("readGenericLiteral")
    }

    pub(crate) fn read_generic_literal1<T>(&mut self, end_exp: impl FnMut(&mut Self) -> PR<T>) -> PR<String> {
        unported!("readGenericLiteral1")
    }

    pub(crate) fn read_generic_escaped(&mut self) -> PR<String> {
        unported!("readGenericEscaped")
    }

    pub(crate) fn read_braced(&mut self) -> PR<Id> {
        unported!("readBraced")
    }
}
