// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checks/Commands.hs `checkBackreferencingDeclaration`, lines 1502-1541 (e11f964)

use std::collections::BTreeMap;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, CommandName};
use shellcheck_analyzer::cfg::{CFEffect, CFNode, IdTagged};
use shellcheck_analyzer::cfg_analysis::CfgAnalysis;
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::arguments;

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LOCAL: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"local\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("local"), local_cmd),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_DECLARE: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"declare\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("declare"), declare_cmd),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_EXPORT: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"export\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("export"), export_cmd),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_READONLY: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"readonly\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("readonly"), readonly_cmd),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_TYPESET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"typeset\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("typeset"), typeset_cmd),
};

#[linkme::distributed_slice(CHECKS)]
static ENTRY_LET: CheckEntry = CheckEntry {
    module: "Checks/Commands.hs",
    name: "checkBackreferencingDeclaration \"let\"",
    registry: "command",
    order: 48,
    optional: None,
    kind: CheckKind::Command(CommandName::Exactly("let"), let_cmd),
};

fn local_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("local", p, t, c)
}

fn declare_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("declare", p, t, c)
}

fn export_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("export", p, t, c)
}

fn readonly_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("readonly", p, t, c)
}

fn typeset_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("typeset", p, t, c)
}

fn let_cmd(p: &Params<'_>, t: Id, c: &mut Comments) {
    check_backreferencing_declaration("let", p, t, c)
}

fn check_backreferencing_declaration(cmd: &'static str, p: &Params<'_>, t: Id, c: &mut Comments) {
    // mapM_ (\cfga -> foldM_ (perArg cfga) M.empty $ arguments t) maybeCfga
    let Some(cfga) = p.cfg_analysis.as_deref() else { return };
    let mut left_args: BTreeMap<String, Id> = BTreeMap::new();
    for arg in arguments(p.ast, t) {
        per_arg(cmd, p, cfga, &mut left_args, arg, c);
    }
}

fn per_arg(
    cmd: &str,
    p: &Params<'_>,
    cfga: &CfgAnalysis,
    left_args: &mut BTreeMap<String, Id>,
    t: Id,
    c: &mut Comments,
) {
    match p.ast.get(t) {
        Token::T_Assignment(_, name, idx, val) => {
            // warnIfBackreferencing cfga leftArgs $ t:idx
            let mut l = vec![*val];
            l.extend(idx.iter().copied());
            warn_if_backreferencing(cmd, cfga, left_args, &l, c);
            left_args.insert(name.clone(), t);
        }
        _ => warn_if_backreferencing(cmd, cfga, left_args, &[t], c),
    }
}

fn warn_if_backreferencing(
    cmd: &str,
    cfga: &CfgAnalysis,
    backrefs: &BTreeMap<String, Id>,
    l: &[Id],
    c: &mut Comments,
) {
    let references = find_references(cfga, l);
    // M.intersection backrefs references (left-biased), in key order
    for (name, id) in backrefs {
        if references.contains_key(name) {
            msg(cmd, name, *id, c);
        }
    }
}

fn msg(cmd: &str, _name: &str, id: Id, c: &mut Comments) {
    c.warn(
        id,
        2318,
        format!(
            "This assignment is used again in this '{cmd}', but won't have taken effect. Use two '{cmd}'s."
        ),
    );
}

fn find_references(cfga: &CfgAnalysis, list: &[Id]) -> BTreeMap<String, Id> {
    let graph = cfga.graph();
    let mut nodes = std::collections::BTreeSet::new();
    for id in list {
        if let Some(ns) = cfga.token_to_nodes(*id) {
            nodes.extend(ns);
        }
    }
    let mut references = BTreeMap::new();
    for n in nodes {
        if let Some(lab) = graph.label(n) {
            // M.fromList: later entries win
            for (name, id) in ref_from_label(lab) {
                references.insert(name, id);
            }
        }
    }
    references
}

fn ref_from_label(lab: &CFNode) -> Vec<(String, Id)> {
    match lab {
        CFNode::CFApplyEffects(effects) => effects.iter().filter_map(ref_from_effect).collect(),
        _ => vec![],
    }
}

fn ref_from_effect(e: &IdTagged<CFEffect>) -> Option<(String, Id)> {
    match e {
        IdTagged(id, CFEffect::CFReadVariable(name)) => Some((name.clone(), *id)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn fires(cmd: &str, script: &str) -> bool {
        let name = format!("checkBackreferencingDeclaration \"{cmd}\"");
        let codes = run_check("Checks/Commands.hs", &name, "node", script).expect("registered");
        codes.contains(&2318)
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration1() {
        assert!(fires("declare", "declare x=1 y=foo$x"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration2() {
        assert!(fires("readonly", "readonly x=1 y=$((1+x))"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration3() {
        assert!(fires("local", "local x=1 y=$(echo $x)"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration4() {
        assert!(fires("local", "local x=1 y[$x]=z"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration5() {
        assert!(fires("declare", "declare x=var $x=1"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration6() {
        assert!(fires("declare", "declare x=var $x=1"));
    }

    #[test]
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_backreferencing_declaration7() {
        assert!(fires("declare", "declare x=var $k=$x"));
    }
}
