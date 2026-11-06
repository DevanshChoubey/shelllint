// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 59-155:
// character classes, spacing, unicode quotes and spaces.

use shellcheck_core::ast::{Id, Token};
use shellcheck_core::data::SPECIAL_VARIABLES;
use shellcheck_core::interface::Severity::{ErrorC, WarningC};

use crate::alt;
use crate::engine::{P, PR};

pub(crate) const QUOTABLE_CHARS: &str = "|&;<>()\\ '\t\n\r\u{A0}\\\"$`"; // ++ doubleQuotableChars
pub(crate) const DOUBLE_QUOTABLE_CHARS: &str = "\\\"$`";
pub(crate) const EXTGLOB_START_CHARS: &str = "?*@!+";
pub(crate) const UNICODE_DOUBLE_QUOTES: &str = "\u{201C}\u{201D}\u{2033}\u{2036}";
pub(crate) const UNICODE_SINGLE_QUOTES: &str = "\u{2018}\u{2019}";

impl<'a> P<'a> {
    pub(crate) fn backslash(&mut self) -> PR<char> {
        self.char_('\\')
    }

    pub(crate) fn linefeed(&mut self) -> PR<char> {
        self.optional(|p| p.carriage_return())?;
        let c = self.char_('\n')?;
        self.read_pending_here_docs()?;
        Ok(c)
    }

    pub(crate) fn single_quote(&mut self) -> PR<char> {
        self.char_('\'')
    }

    pub(crate) fn double_quote(&mut self) -> PR<char> {
        self.char_('"')
    }

    pub(crate) fn variable_start(&mut self) -> PR<char> {
        alt!(self, |p| p.upper(), |p| p.lower(), |p| p.one_of("_"))
    }

    pub(crate) fn variable_chars(&mut self) -> PR<char> {
        alt!(self, |p| p.upper(), |p| p.lower(), |p| p.digit(), |p| p.one_of("_"))
    }

    /// Chars to allow function names to start with
    pub(crate) fn function_start_chars(&mut self) -> PR<char> {
        alt!(self, |p| p.variable_chars(), |p| p.one_of(":+?-./^@,"))
    }

    /// Chars to allow inside function names
    pub(crate) fn function_chars(&mut self) -> PR<char> {
        alt!(self, |p| p.variable_chars(), |p| p.one_of("#:+?-./^@,"))
    }

    /// Chars to allow function names to start with, using the 'function' keyword
    pub(crate) fn extended_function_start_chars(&mut self) -> PR<char> {
        alt!(self, |p| p.function_start_chars(), |p| p.one_of("[]*=!"))
    }

    /// Chars to allow in functions using the 'function' keyword
    pub(crate) fn extended_function_chars(&mut self) -> PR<char> {
        alt!(self, |p| p.extended_function_start_chars(), |p| p.one_of("[]*=!"))
    }

    pub(crate) fn special_variable(&mut self) -> PR<char> {
        // oneOf (concat specialVariables)
        self.satisfy(|c| SPECIAL_VARIABLES.iter().any(|v| v.contains(c)))
    }

    pub(crate) fn param_sub_special_chars(&mut self) -> PR<char> {
        self.one_of("/:+-=%")
    }

    pub(crate) fn quotable(&mut self) -> PR<char> {
        alt!(self, |p| p.almost_space(), |p| p.one_of(QUOTABLE_CHARS))
    }

    pub(crate) fn braced_quotable(&mut self) -> PR<char> {
        self.one_of("}\"$`'")
    }

    pub(crate) fn double_quotable(&mut self) -> PR<char> {
        self.one_of(DOUBLE_QUOTABLE_CHARS)
    }

    pub(crate) fn whitespace(&mut self) -> PR<char> {
        alt!(
            self,
            |p| p.one_of(" \t"),
            |p| p.carriage_return(),
            |p| p.almost_space(),
            |p| p.linefeed()
        )
    }

    pub(crate) fn linewhitespace(&mut self) -> PR<char> {
        alt!(self, |p| p.one_of(" \t"), |p| p.almost_space())
    }

    pub(crate) fn suspect_char_after_quotes(&mut self) -> PR<char> {
        alt!(self, |p| p.variable_chars(), |p| p.char_('%'))
    }

    pub(crate) fn extglob_start(&mut self) -> PR<char> {
        self.one_of(EXTGLOB_START_CHARS)
    }

    pub(crate) fn spacing(&mut self) -> PR<String> {
        let x = self.many(|p| {
            p.alt(
                |p| p.many1(|p| p.linewhitespace()),
                |p| {
                    // continuation
                    p.try_(|p| p.string("\\\n"))?;
                    // The line was continued. Warn if this next line is a comment with a trailing \
                    let whitespace = p.many(|p| p.linewhitespace())?;
                    p.optional(|p| {
                        let x = p.read_comment()?;
                        if x.ends_with('\\') {
                            p.parse_problem(
                                ErrorC,
                                1143,
                                "This backslash is part of a comment and does not continue the line.",
                            );
                        }
                        Ok(())
                    })?;
                    Ok(whitespace)
                },
            )
        })?;
        self.optional(|p| p.read_comment())?;
        Ok(x.into_iter().flatten().collect())
    }

    pub(crate) fn spacing1(&mut self) -> PR<String> {
        let spacing = self.spacing()?;
        if spacing.is_empty() {
            return self.fail("Expected whitespace");
        }
        Ok(spacing)
    }

    pub(crate) fn allspacing(&mut self) -> PR<String> {
        let s = self.spacing()?;
        let more = self.option(false, |p| {
            p.linefeed()?;
            Ok(true)
        })?;
        if more {
            let rest = self.allspacing()?;
            Ok(format!("{s}\n{rest}"))
        } else {
            Ok(s)
        }
    }

    pub(crate) fn allspacing_or_fail(&mut self) -> PR<String> {
        let s = self.allspacing()?;
        if s.is_empty() {
            return self.fail("Expected whitespace");
        }
        Ok(s)
    }

    pub(crate) fn read_unicode_quote(&mut self) -> PR<Id> {
        let start = self.start_span();
        let c = self.one_of(&format!("{UNICODE_SINGLE_QUOTES}{UNICODE_DOUBLE_QUOTES}"))?;
        let id = self.end_span(start);
        self.parse_problem_at_id(
            id,
            WarningC,
            1110,
            "This is a unicode quote. Delete and retype it (or quote to make literal).",
        );
        Ok(self.node(id, Token::T_Literal(c.to_string())))
    }

    pub(crate) fn carriage_return(&mut self) -> PR<char> {
        let pos = self.get_position();
        self.char_('\r')?;
        self.parse_problem_at(pos, ErrorC, 1017, "Literal carriage return. Run script through tr -d '\\r' .");
        Ok('\r')
    }

    pub(crate) fn almost_space(&mut self) -> PR<char> {
        self.parse_note(ErrorC, 1018, "This is a unicode space. Delete and retype it.");
        self.one_of("\u{A0}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200B}\u{202F}")?;
        Ok(' ')
    }
}
