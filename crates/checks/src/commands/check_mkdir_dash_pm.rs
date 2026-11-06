// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkMkdirDashPM`, lines 635-649 (e11f964)

use std::sync::LazyLock;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_all_flags, get_literal_string};
use regex::Regex;
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkMkdirDashPM",
    registry: "command",
    order: 11,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("mkdir"), check_mkdir_dash_pm),
};

fn check_mkdir_dash_pm(p: &Params<'_>, t: Id, c: &mut Comments) {
    let flags = get_all_flags(p.ast, t);
    let Some(_dash_p) = flags.iter().find(|(_, f)| f == "p" || f == "parents") else {
        return;
    };
    let Some(dash_m) = flags.iter().find(|(_, f)| f == "m" || f == "mode") else {
        return;
    };
    // mkdir -pm 0700 dir  is fine, so is ../dir, but dir/subdir is not.
    if !arguments(p.ast, t).iter().skip(1).any(|&a| could_have_subdirs(p, a)) {
        return;
    }
    c.warn(dash_m.0, 2174, "When used with -p, -m only applies to the deepest directory.");
}

fn could_have_subdirs(p: &Params<'_>, t: Id) -> bool {
    match get_literal_string(p.ast, t) {
        None => true,
        Some(name) => name.contains('/') && !matches(&name, &RE),
    }
}

static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^(\\.\\.?\\/)+[^/]+$"));
