// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/ShellSupport.hs `checkForDecimals`, lines 80-88 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_literal_string;
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkForDecimals",
    registry: "shell-support",
    order: 0,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Sh, Shell::Dash, Shell::BusyboxSh, Shell::Bash], f),
};

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::TA_Expansion(_) = p.ast.get(t) else { return };
    // `sequence_ $ do ...` in the Maybe monad: any failed step is a no-op.
    let Some(s) = get_literal_string(p.ast, t) else { return };
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return };
    if first.is_ascii_digit() && chars.as_str().contains('.') {
        c.err(t, 2079, "(( )) doesn't support decimals. Use bc or awk.");
    }
}
