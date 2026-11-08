// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkXargsDashi`, lines 1346-1354 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind, CommandName};
use shelllint_core::ast::Id;
use shelllint_core::astlib::{arguments, get_bsd_opts};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
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

fn parse_opts(p: &Params<'_>, args: &[Id]) -> Option<Vec<shelllint_core::astlib::Opt>> {
    get_bsd_opts(p.ast, "0oprtxadR:S:J:L:l:n:P:s:e:E:i:I:", args)
}
