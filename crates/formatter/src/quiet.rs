// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Formatter/Quiet.hs (e11f964)

use std::io::Write;

use shelllint_core::interface::{CheckResult, FormatterOptions, SystemInterface};

use crate::Formatter;

#[derive(Default)]
pub struct Quiet {
    failed: bool,
}

pub fn format(_options: FormatterOptions) -> Quiet {
    Quiet::default()
}

impl Formatter for Quiet {
    fn header(&mut self, _out: &mut dyn Write) {}
    fn footer(&mut self, _out: &mut dyn Write) {}

    fn on_failure(&mut self, _file: &str, _msg: &str, _out: &mut dyn Write) {
        self.failed = true;
    }

    fn on_result(&mut self, result: &CheckResult, _sys: &dyn SystemInterface, _out: &mut dyn Write) {
        if !result.comments.is_empty() {
            self.failed = true;
        }
    }

    fn exit_requested(&self) -> bool {
        self.failed
    }
}
