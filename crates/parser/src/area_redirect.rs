// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1836-2053:
// here documents, pending here documents, IO redirections, here strings.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_here_doc(&mut self) -> PR<Id> {
        unported!("readHereDoc")
    }

    pub(crate) fn read_pending_here_docs(&mut self) -> PR<()> {
        unported!("readPendingHereDocs")
    }

    pub(crate) fn read_filename(&mut self) -> PR<Id> {
        unported!("readFilename")
    }

    pub(crate) fn read_io_file_op(&mut self) -> PR<Id> {
        unported!("readIoFileOp")
    }

    pub(crate) fn read_io_duplicate(&mut self) -> PR<Id> {
        unported!("readIoDuplicate")
    }

    pub(crate) fn read_io_file(&mut self) -> PR<Id> {
        unported!("readIoFile")
    }

    pub(crate) fn read_io_variable(&mut self) -> PR<String> {
        unported!("readIoVariable")
    }

    pub(crate) fn read_io_source(&mut self) -> PR<String> {
        unported!("readIoSource")
    }

    pub(crate) fn read_io_redirect(&mut self) -> PR<Id> {
        unported!("readIoRedirect")
    }

    pub(crate) fn read_here_string(&mut self) -> PR<Id> {
        unported!("readHereString")
    }
}
