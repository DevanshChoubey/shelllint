// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2054-2438:
// separators, simple commands, `source`, pipelines, and/or lists, terms.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR, SourcePos};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_newline_list(&mut self) -> PR<String> {
        unported!("readNewlineList")
    }

    pub(crate) fn read_line_break(&mut self) -> PR<()> {
        unported!("readLineBreak")
    }

    /// The separator char and its (start, end) span.
    pub(crate) fn read_separator_op(&mut self) -> PR<(char, (SourcePos, SourcePos))> {
        unported!("readSeparatorOp")
    }

    pub(crate) fn read_sequential_sep(&mut self) -> PR<()> {
        unported!("readSequentialSep")
    }

    pub(crate) fn read_separator(&mut self) -> PR<(char, (SourcePos, SourcePos))> {
        unported!("readSeparator")
    }

    pub(crate) fn read_simple_command(&mut self) -> PR<Id> {
        unported!("readSimpleCommand")
    }

    pub(crate) fn read_source(&mut self, t: Id) -> PR<Id> {
        unported!("readSource")
    }

    pub(crate) fn read_pipeline(&mut self) -> PR<Id> {
        unported!("readPipeline")
    }

    /// `parser` is a `dyn` reference because `readBanged` recurses with it.
    pub(crate) fn read_banged(&mut self, parser: &mut dyn FnMut(&mut Self) -> PR<Id>) -> PR<Id> {
        unported!("readBanged")
    }

    pub(crate) fn read_and_or(&mut self) -> PR<Id> {
        unported!("readAndOr")
    }

    pub(crate) fn read_term_or_none(&mut self) -> PR<Vec<Id>> {
        unported!("readTermOrNone")
    }

    pub(crate) fn read_term(&mut self) -> PR<Vec<Id>> {
        unported!("readTerm")
    }

    pub(crate) fn read_pipe_sequence(&mut self) -> PR<Id> {
        unported!("readPipeSequence")
    }

    pub(crate) fn read_pipe(&mut self) -> PR<Id> {
        unported!("readPipe")
    }

    pub(crate) fn read_command(&mut self) -> PR<Id> {
        unported!("readCommand")
    }

    pub(crate) fn read_cmd_name(&mut self) -> PR<Id> {
        unported!("readCmdName")
    }

    pub(crate) fn read_cmd_word(&mut self) -> PR<Id> {
        unported!("readCmdWord")
    }

    /// Due to poor planning, annotations after commands isn't handled well.
    pub(crate) fn skip_annotation_and_warn(&mut self) -> PR<()> {
        unported!("skipAnnotationAndWarn")
    }
}
