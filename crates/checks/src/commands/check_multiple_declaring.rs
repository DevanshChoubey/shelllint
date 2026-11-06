// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMultipleDeclaring`, lines 1038-1046 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_command_token_or_this, get_unquoted_literal};
use shellcheck_core::data::DECLARING_COMMANDS;

use crate::support::parametrised_command_checks;

parametrised_command_checks!(
    "checkMultipleDeclaring",
    47,
    Exactly,
    check_multiple_declaring,
    ["local", "declare", "export", "readonly", "typeset", "let"]
);

/// `mapM_ check . arguments`
fn check_multiple_declaring(cmd: &'static str, p: &Params<'_>, t: Id, c: &mut Comments) {
    for a in arguments(p.ast, t) {
        let Some(lit) = get_unquoted_literal(p.ast, a) else { continue };
        if !DECLARING_COMMANDS.contains(&lit.as_str()) {
            continue;
        }
        // Haskell's `check t` shadows `t` with the argument.
        c.err(
            get_command_token_or_this(p.ast, a),
            2316,
            format!(
                "This applies {cmd} to the variable named {lit}, which is probably not what you want. Use a separate command or the appropriate `declare` options instead."
            ),
        );
    }
}

