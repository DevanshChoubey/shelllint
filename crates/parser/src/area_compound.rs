// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2439-2908:
// if/while/until/for/select/case, functions, coproc, subshells, brace groups,
// bats tests, compound commands and lists.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::{CaseType, Id};

use crate::engine::{P, PR};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_if_clause(&mut self) -> PR<Id> {
        unported!("readIfClause")
    }

    pub(crate) fn verify_not_empty_if(&mut self, s: &str) -> PR<()> {
        unported!("verifyNotEmptyIf")
    }

    /// (condition, action)
    pub(crate) fn read_if_part(&mut self) -> PR<(Vec<Id>, Vec<Id>)> {
        unported!("readIfPart")
    }

    /// (condition, action)
    pub(crate) fn read_elif_part(&mut self) -> PR<(Vec<Id>, Vec<Id>)> {
        unported!("readElifPart")
    }

    pub(crate) fn read_else_part(&mut self) -> PR<Vec<Id>> {
        unported!("readElsePart")
    }

    pub(crate) fn if_next_token<T>(
        &mut self,
        parser: impl FnOnce(&mut Self) -> PR<T>,
        action: impl FnOnce(&mut Self) -> PR<()>,
    ) -> PR<()> {
        unported!("ifNextToken")
    }

    pub(crate) fn read_subshell(&mut self) -> PR<Id> {
        unported!("readSubshell")
    }

    pub(crate) fn read_brace_group(&mut self) -> PR<Id> {
        unported!("readBraceGroup")
    }

    pub(crate) fn read_bats_test(&mut self) -> PR<Id> {
        unported!("readBatsTest")
    }

    pub(crate) fn read_while_clause(&mut self) -> PR<Id> {
        unported!("readWhileClause")
    }

    pub(crate) fn read_until_clause(&mut self) -> PR<Id> {
        unported!("readUntilClause")
    }

    pub(crate) fn read_do_group(&mut self, kw_id: Id) -> PR<Vec<Id>> {
        unported!("readDoGroup")
    }

    pub(crate) fn read_for_clause(&mut self) -> PR<Id> {
        unported!("readForClause")
    }

    pub(crate) fn read_select_clause(&mut self) -> PR<Id> {
        unported!("readSelectClause")
    }

    pub(crate) fn read_in_clause(&mut self) -> PR<Vec<Id>> {
        unported!("readInClause")
    }

    pub(crate) fn read_case_clause(&mut self) -> PR<Id> {
        unported!("readCaseClause")
    }

    pub(crate) fn read_case_list(&mut self) -> PR<Vec<(CaseType, Vec<Id>, Vec<Id>)>> {
        unported!("readCaseList")
    }

    /// (separator, pattern, list)
    pub(crate) fn read_case_item(&mut self) -> PR<(CaseType, Vec<Id>, Vec<Id>)> {
        unported!("readCaseItem")
    }

    pub(crate) fn read_case_separator(&mut self) -> PR<CaseType> {
        unported!("readCaseSeparator")
    }

    pub(crate) fn read_function_definition(&mut self) -> PR<Id> {
        unported!("readFunctionDefinition")
    }

    pub(crate) fn read_co_proc(&mut self) -> PR<Id> {
        unported!("readCoProc")
    }

    pub(crate) fn read_pattern(&mut self) -> PR<Vec<Id>> {
        unported!("readPattern")
    }

    pub(crate) fn read_condition_command(&mut self) -> PR<Id> {
        unported!("readConditionCommand")
    }

    pub(crate) fn read_compound_command(&mut self) -> PR<Id> {
        unported!("readCompoundCommand")
    }

    pub(crate) fn read_compound_list(&mut self) -> PR<Vec<Id>> {
        unported!("readCompoundList")
    }

    pub(crate) fn read_compound_list_or_empty(&mut self) -> PR<Vec<Id>> {
        unported!("readCompoundListOrEmpty")
    }
}
