// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMaskedReturns`, lines 1408-1448 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, get_path_m};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{arguments, get_all_flags, get_command_name, get_word_parts};
use shellcheck_core::interface::Shell;

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LOCAL: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"local\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("local"), local_),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_DECLARE: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"declare\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("declare"), declare),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_EXPORT: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"export\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("export"), export),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_READONLY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"readonly\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("readonly"), readonly),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_TYPESET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"typeset\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("typeset"), typeset),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMaskedReturns \"let\"",
    registry: "command",
    order: 46,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("let"), let_),
};

fn local_(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn declare(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn export(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn readonly(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn typeset(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_cmd(p, t, c)
}

fn let_(p: &Params<'_>, t: Id, c: &mut Comments) {
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
