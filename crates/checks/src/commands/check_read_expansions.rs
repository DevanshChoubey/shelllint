// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkReadExpansions`, lines 881-909 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{arguments, get_gnu_opts, get_word_parts, is_variable_name};
use shellcheck_core::data::FLAGS_FOR_READ;

use crate::support::get_single_unmodified_braced_string;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkReadExpansions",
    registry: "command",
    order: 36,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("read"), check_read_expansions),
};

fn check_read_expansions(p: &Params<'_>, cmd: Id, c: &mut Comments) {
    for t in get_vars(p, cmd) {
        dollar_warning(p, t, c);
    }
    for w in arguments(p.ast, cmd) {
        array_warning(p, w, c);
    }
}

fn get_vars(p: &Params<'_>, cmd: Id) -> Vec<Id> {
    match get_gnu_opts(p.ast, FLAGS_FOR_READ, &arguments(p.ast, cmd)) {
        Some(opts) => opts
            .into_iter()
            .filter(|(x, _)| x.is_empty() || x == "a")
            .map(|(_, (_, y))| y)
            .collect(),
        None => vec![],
    }
}

fn dollar_warning(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Some(name) = get_single_unmodified_braced_string(p.ast, t) else {
        return;
    };
    // e.g. not $1
    if !is_variable_name(&name) {
        return;
    }
    c.warn(
        t,
        2229,
        &format!("This does not read '{name}'. Remove $/${{}} for that, or use ${{var?}} to quiet."),
    );
}

fn array_warning(p: &Params<'_>, word: Id, c: &mut Comments) {
    if get_word_parts(p.ast, word).into_iter().any(|t| is_unquoted_bracket(p, t)) {
        c.warn(word, 2313, "Quote array indices to avoid them expanding as globs.");
    }
}

fn is_unquoted_bracket(p: &Params<'_>, t: Id) -> bool {
    matches!(p.ast.get(t), Token::T_Glob(s) if s.starts_with('['))
}
