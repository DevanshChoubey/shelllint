// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkXargsDashi`, lines 1346-1354 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{arguments, get_bsd_opts};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkXargsDashi",
    registry: "command",
    order: 39,
    optional: None,
    kind: CheckKind::Command(CommandName::Basename("xargs"), check_xargs_dashi),
};

fn check_xargs_dashi(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Some(opts) = parse_opts(p, &arguments(p.ast, t)) else { return };
    let Some((_, (option, _value))) = opts.iter().find(|(k, _)| k == "i") else { return };
    c.info(*option, 2267, "GNU xargs -i is deprecated in favor of -I{}");
}

fn parse_opts(p: &Params<'_>, args: &[Id]) -> Option<Vec<shellcheck_core::astlib::Opt>> {
    get_bsd_opts(p.ast, "0oprtxadR:S:J:L:l:n:P:s:e:E:i:I:", args)
}
