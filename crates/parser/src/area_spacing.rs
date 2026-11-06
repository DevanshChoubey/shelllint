// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 59-155:
// character classes, spacing, unicode quotes and spaces.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR};
use crate::unported;

pub(crate) const QUOTABLE_CHARS: &str = "|&;<>()\\ '\t\n\r\u{A0}\\\"$`"; // ++ doubleQuotableChars
pub(crate) const DOUBLE_QUOTABLE_CHARS: &str = "\\\"$`";
pub(crate) const EXTGLOB_START_CHARS: &str = "?*@!+";
pub(crate) const UNICODE_DOUBLE_QUOTES: &str = "\u{201C}\u{201D}\u{2033}\u{2036}";
pub(crate) const UNICODE_SINGLE_QUOTES: &str = "\u{2018}\u{2019}";

impl<'a> P<'a> {
    pub(crate) fn backslash(&mut self) -> PR<char> {
        unported!("backslash")
    }

    pub(crate) fn linefeed(&mut self) -> PR<char> {
        unported!("linefeed")
    }

    pub(crate) fn single_quote(&mut self) -> PR<char> {
        unported!("singleQuote")
    }

    pub(crate) fn double_quote(&mut self) -> PR<char> {
        unported!("doubleQuote")
    }

    pub(crate) fn variable_start(&mut self) -> PR<char> {
        unported!("variableStart")
    }

    pub(crate) fn variable_chars(&mut self) -> PR<char> {
        unported!("variableChars")
    }

    /// Chars to allow function names to start with
    pub(crate) fn function_start_chars(&mut self) -> PR<char> {
        unported!("functionStartChars")
    }

    /// Chars to allow inside function names
    pub(crate) fn function_chars(&mut self) -> PR<char> {
        unported!("functionChars")
    }

    /// Chars to allow function names to start with, using the 'function' keyword
    pub(crate) fn extended_function_start_chars(&mut self) -> PR<char> {
        unported!("extendedFunctionStartChars")
    }

    /// Chars to allow in functions using the 'function' keyword
    pub(crate) fn extended_function_chars(&mut self) -> PR<char> {
        unported!("extendedFunctionChars")
    }

    pub(crate) fn special_variable(&mut self) -> PR<char> {
        unported!("specialVariable")
    }

    pub(crate) fn param_sub_special_chars(&mut self) -> PR<char> {
        unported!("paramSubSpecialChars")
    }

    pub(crate) fn quotable(&mut self) -> PR<char> {
        unported!("quotable")
    }

    pub(crate) fn braced_quotable(&mut self) -> PR<char> {
        unported!("bracedQuotable")
    }

    pub(crate) fn double_quotable(&mut self) -> PR<char> {
        unported!("doubleQuotable")
    }

    pub(crate) fn whitespace(&mut self) -> PR<char> {
        unported!("whitespace")
    }

    pub(crate) fn linewhitespace(&mut self) -> PR<char> {
        unported!("linewhitespace")
    }

    pub(crate) fn suspect_char_after_quotes(&mut self) -> PR<char> {
        unported!("suspectCharAfterQuotes")
    }

    pub(crate) fn extglob_start(&mut self) -> PR<char> {
        unported!("extglobStart")
    }

    pub(crate) fn spacing(&mut self) -> PR<String> {
        unported!("spacing")
    }

    pub(crate) fn spacing1(&mut self) -> PR<String> {
        unported!("spacing1")
    }

    pub(crate) fn allspacing(&mut self) -> PR<String> {
        unported!("allspacing")
    }

    pub(crate) fn allspacing_or_fail(&mut self) -> PR<String> {
        unported!("allspacingOrFail")
    }

    pub(crate) fn read_unicode_quote(&mut self) -> PR<Id> {
        unported!("readUnicodeQuote")
    }

    pub(crate) fn carriage_return(&mut self) -> PR<char> {
        unported!("carriageReturn")
    }

    pub(crate) fn almost_space(&mut self) -> PR<char> {
        unported!("almostSpace")
    }
}
