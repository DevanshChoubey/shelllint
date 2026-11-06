// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1612-1835:
// all $ expansions, variable names.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR, SourcePos};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn ensure_dollar(&mut self) -> PR<char> {
        unported!("ensureDollar")
    }

    pub(crate) fn read_normal_dollar(&mut self) -> PR<Id> {
        unported!("readNormalDollar")
    }

    pub(crate) fn read_double_quoted_dollar(&mut self) -> PR<Id> {
        unported!("readDoubleQuotedDollar")
    }

    pub(crate) fn read_dollar_expression(&mut self) -> PR<Id> {
        unported!("readDollarExpression")
    }

    pub(crate) fn read_dollar_exp(&mut self) -> PR<Id> {
        unported!("readDollarExp")
    }

    pub(crate) fn read_dollar_single_quote(&mut self) -> PR<Id> {
        unported!("readDollarSingleQuote")
    }

    pub(crate) fn read_dollar_double_quote(&mut self) -> PR<Id> {
        unported!("readDollarDoubleQuote")
    }

    pub(crate) fn read_dollar_arithmetic(&mut self) -> PR<Id> {
        unported!("readDollarArithmetic")
    }

    pub(crate) fn read_dollar_bracket(&mut self) -> PR<Id> {
        unported!("readDollarBracket")
    }

    pub(crate) fn read_arithmetic_expression(&mut self) -> PR<Id> {
        unported!("readArithmeticExpression")
    }

    /// If the next characters match prefix, try two different parsers and warn if the alternate parser had to be used
    pub(crate) fn read_ambiguous<T>(
        &mut self,
        prefix: &str,
        expected: impl FnMut(&mut Self) -> PR<T>,
        alternative: impl FnMut(&mut Self) -> PR<T>,
        warner: impl FnMut(&mut Self, SourcePos) -> PR<()>,
    ) -> PR<T> {
        unported!("readAmbiguous")
    }

    pub(crate) fn read_dollar_brace_command_expansion(&mut self) -> PR<Id> {
        unported!("readDollarBraceCommandExpansion")
    }

    pub(crate) fn read_dollar_braced(&mut self) -> PR<Id> {
        unported!("readDollarBraced")
    }

    pub(crate) fn read_dollar_expansion(&mut self) -> PR<Id> {
        unported!("readDollarExpansion")
    }

    pub(crate) fn read_dollar_variable(&mut self) -> PR<Id> {
        unported!("readDollarVariable")
    }

    pub(crate) fn read_variable_name(&mut self) -> PR<String> {
        unported!("readVariableName")
    }

    pub(crate) fn read_dollar_lonely(&mut self, quoted: bool) -> PR<Id> {
        unported!("readDollarLonely")
    }
}
