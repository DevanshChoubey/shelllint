// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkNonportableSignals`, lines 657-684 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_literal_string, is_flag};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkNonportableSignals",
    registry: "command",
    order: 12,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("trap"), check_nonportable_signals),
};

fn check_nonportable_signals(p: &Params<'_>, t: Id, c: &mut Comments) {
    let args = arguments(p.ast, t);
    match args.split_first() {
        Some((first, rest)) if !is_flag(p.ast, *first) => {
            for param in rest {
                check(p, *param, c);
            }
        }
        _ => {}
    }
}

fn check(p: &Params<'_>, param: Id, c: &mut Comments) {
    let Some(s) = get_literal_string(p.ast, param) else {
        return;
    };
    // sequence_ over the Maybe results: every applicable check fires.
    check_numeric(param, &s, c);
    check_untrappable(param, &s, c);
}

fn check_numeric(id: Id, s: &str, c: &mut Comments) {
    if s.is_empty() || !s.chars().all(|ch| ch.is_ascii_digit()) {
        return;
    }
    if s == "0" {
        return; // POSIX exit trap
    }
    if ["1", "2", "3", "6", "9", "14", "15"].contains(&s) {
        return; // XSI
    }
    c.warn(id, 2172, "Trapping signals by number is not well defined. Prefer signal names.");
}

fn check_untrappable(id: Id, s: &str, c: &mut Comments) {
    let lower = s.to_lowercase();
    if ["kill", "9", "sigkill", "stop", "sigstop"].contains(&lower.as_str()) {
        c.err(id, 2173, "SIGKILL/SIGSTOP can not be trapped.");
    }
}
