// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkArgComparison`, lines 1363-1392 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{
    arguments, get_leading_unquoted_string, get_literal_string, get_trailing_unquoted_literal,
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_ALIAS: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"alias\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("alias"), cmd_alias),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LOCAL: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"local\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("local"), cmd_local),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_DECLARE: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"declare\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("declare"), cmd_declare),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_EXPORT: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"export\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("export"), cmd_export),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_READONLY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"readonly\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("readonly"), cmd_readonly),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_TYPESET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"typeset\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("typeset"), cmd_typeset),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkArgComparison \"let\"",
    registry: "command",
    order: 45,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("let"), cmd_let),
};

fn cmd_alias(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("alias", p, t, c)
}

fn cmd_local(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("local", p, t, c)
}

fn cmd_declare(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("declare", p, t, c)
}

fn cmd_export(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("export", p, t, c)
}

fn cmd_readonly(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("readonly", p, t, c)
}

fn cmd_typeset(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("typeset", p, t, c)
}

fn cmd_let(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_arg_comparison("let", p, t, c)
}

fn check_arg_comparison(cmd: &str, p: &Params<'_>, t: Id, c: &mut Comments) {
    for arg in arguments(p.ast, t) {
        check(cmd, p, arg, c);
    }
}

fn check(cmd: &str, p: &Params<'_>, arg: Id, c: &mut Comments) {
    if let Some(s) = get_leading_unquoted_string(p.ast, arg) {
        if s.starts_with('=') {
            c.err(head_id(p, arg), 2290, "Remove spaces around = to assign.");
        } else if s.starts_with("+=") {
            c.err(head_id(p, arg), 2290, "Remove spaces around += to append.");
        }
    }

    // 'let' is parsed as a sequence of arithmetic expansions,
    // so we want the additional warning for "x="
    if cmd == "let" {
        if let Some(token) = get_trailing_unquoted_literal(p.ast, arg) {
            if let Some(s) = get_literal_string(p.ast, token) {
                if s.ends_with('=') {
                    c.err(token, 2290, "Remove spaces around = to assign.");
                }
            }
        }
    }
}

fn head_id(p: &Params<'_>, t: Id) -> Id {
    match p.ast.get(t) {
        Token::T_NormalWord(l) if !l.is_empty() => l[0],
        _ => t,
    }
}
