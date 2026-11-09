// SPDX-License-Identifier: GPL-3.0-or-later
// `--summary` and `--fail-fast` (extensions beyond ShellCheck).

use shelllint_core::interface::{CheckResult, Severity};

/// Running totals over the files of one run.
#[derive(Default)]
pub(crate) struct Tally {
    files: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
    styles: usize,
}

impl Tally {
    /// Records one checked file. Returns the number of findings in it.
    pub(crate) fn record(&mut self, result: &CheckResult) -> usize {
        self.files += 1;
        for pc in &result.comments {
            match pc.comment.severity {
                Severity::ErrorC => self.errors += 1,
                Severity::WarningC => self.warnings += 1,
                Severity::InfoC => self.infos += 1,
                Severity::StyleC => self.styles += 1,
            }
        }
        result.comments.len()
    }

    /// The `--summary` line, without a trailing newline.
    pub(crate) fn summary(&self) -> String {
        format!(
            "{} file(s) checked: {} error(s), {} warning(s), {} info(s), {} style(s)",
            self.files, self.errors, self.warnings, self.infos, self.styles
        )
    }
}
