// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 713-917:
// readArithmeticContents and its precedence ladder.
#![allow(dead_code, unused_variables)] // stubs; remove once this area is ported

use shellcheck_core::ast::Id;

use crate::engine::{P, PR};
use crate::unported;

impl<'a> P<'a> {
    pub(crate) fn read_arithmetic_contents(&mut self) -> PR<Id> {
        unported!("readArithmeticContents")
    }
}
