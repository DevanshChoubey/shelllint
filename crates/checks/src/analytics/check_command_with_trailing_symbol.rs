// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkCommandWithTrailingSymbol`, lines 4831-4857 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_literal_string_def;
use shelllint_core::prelude::last_or_default;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkCommandWithTrailingSymbol",
    registry: "node",
    order: 92,
    optional: None,
    kind: CheckKind::Node(check_command_with_trailing_symbol),
};

fn check_command_with_trailing_symbol(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(_, words) = p.ast.get(t) else { return };
    let Some(&cmd) = words.first() else { return };
    let s = get_literal_string_def(p.ast, "x", cmd);
    let last = last_or_default('x', &s.chars().collect::<Vec<_>>());
    match s.as_str() {
        "." | ":" | " " | "//" => {}
        "" => c.err(cmd, 2286, "This empty string is interpreted as a command name. Double check syntax (or use 'true' as a no-op)."),
        _ if last == '/' => c.err(cmd, 2287, "This is interpreted as a command name ending with '/'. Double check syntax."),
        _ if "\\.,([{<>}])#\"'% ".contains(last) => c.warn(
            cmd,
            2288,
            format!("This is interpreted as a command name ending with {}. Double check syntax.", format(last)),
        ),
        _ if s.contains('\t') => c.err(cmd, 2289, "This is interpreted as a command name containing a tab. Double check syntax."),
        _ if s.contains('\n') => c.err(cmd, 2289, "This is interpreted as a command name containing a linefeed. Double check syntax."),
        _ => {}
    }
}

fn format(x: char) -> String {
    match x {
        ' ' => "space".to_string(),
        '\'' => "apostrophe".to_string(),
        '"' => "doublequote".to_string(),
        x => format!("'{}'", x),
    }
}
