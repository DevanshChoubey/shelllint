// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMaskedReturns`, lines 1408-1448 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_path_m};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{arguments, get_all_flags, get_command_name, get_word_parts};
use shellcheck_core::interface::Shell;

use crate::support::parametrised_command_checks;

parametrised_command_checks!(
    "checkMaskedReturns",
    46,
    Exactly,
    check_masked_returns,
    ["local", "declare", "export", "readonly", "typeset", "let"]
);

/// `checkMaskedReturns str = CommandCheck (Exactly str) checkCmd`
fn check_masked_returns(_str: &str, p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn check_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    let path = get_path_m(p, t);
    let shell = p.shell_type;
    let Some(name) = get_command_name(p.ast, t) else { return };

    let flags: Vec<String> = get_all_flags(p.ast, t).into_iter().map(|(_, f)| f).collect();
    let has_dash_r = flags.iter().any(|f| f == "r");
    let has_dash_g = flags.iter().any(|f| f == "g");
    let is_in_scoped_function = path.iter().any(|&x| is_scoped_function(p, shell, x));

    let is_local = !has_dash_g && is_local_in_function(&name) && is_in_scoped_function;
    let is_read_only = name == "readonly" || has_dash_r;

    // Don't warn about local variables that are declared readonly,
    // because the workaround `local x; x=$(false); local -r x;` is annoying
    if is_local && is_read_only {
        return;
    }

    for a in arguments(p.ast, t) {
        check_args(p, a, c);
    }
}

fn check_args(p: &Params<'_>, t: Id, c: &mut Comments) {
    if let Token::T_Assignment(_, _, _, word) = p.ast.get(t) {
        if get_word_parts(p.ast, *word).into_iter().any(|x| has_return(p, x)) {
            c.warn(t, 2155, "Declare and assign separately to avoid masking return values.");
        }
    }
}

fn is_local_in_function(n: &str) -> bool {
    ["local", "declare", "typeset"].contains(&n)
}

fn is_scoped_function(p: &Params<'_>, shell: Shell, t: Id) -> bool {
    match p.ast.get(t) {
        Token::T_BatsTest(..) => true,
        // In ksh, only functions declared with 'function' have their own scope
        Token::T_Function(kw, ..) => shell != Shell::Ksh || kw.0,
        _ => false,
    }
}

fn has_return(p: &Params<'_>, t: Id) -> bool {
    matches!(
        p.ast.get(t),
        Token::T_Backticked(..) | Token::T_DollarExpansion(..) | Token::T_DollarBraceCommandExpansion(..)
    )
}
