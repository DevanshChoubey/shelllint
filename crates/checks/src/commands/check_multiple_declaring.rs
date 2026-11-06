// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMultipleDeclaring`, lines 1038-1046 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_command_token_or_this, get_unquoted_literal};
use shellcheck_core::data::DECLARING_COMMANDS;

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LOCAL: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"local\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("local"), w_local),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_DECLARE: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"declare\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("declare"), w_declare),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_EXPORT: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"export\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("export"), w_export),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_READONLY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"readonly\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("readonly"), w_readonly),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_TYPESET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"typeset\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("typeset"), w_typeset),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMultipleDeclaring \"let\"",
    registry: "command",
    order: 47,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("let"), w_let),
};

/// `mapM_ check . arguments`
fn check_multiple_declaring(cmd: &'static str, p: &Params<'_>, t: Id, c: &mut Comments) {
    for a in arguments(p.ast, t) {
        let Some(lit) = get_unquoted_literal(p.ast, a) else { continue };
        if !DECLARING_COMMANDS.contains(&lit.as_str()) {
            continue;
        }
        c.err(
            get_command_token_or_this(p.ast, t),
            2316,
            format!(
                "This applies {cmd} to the variable named {lit}, which is probably not what you want. Use a separate command or the appropriate `declare` options instead."
            ),
        );
    }
}

fn w_local(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("local", p, t, c)
}

fn w_declare(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("declare", p, t, c)
}

fn w_export(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("export", p, t, c)
}

fn w_readonly(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("readonly", p, t, c)
}

fn w_typeset(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("typeset", p, t, c)
}

fn w_let(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_multiple_declaring("let", p, t, c)
}
