// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkFunctionsUsedExternally`, lines 2454-2506 (e11f964)

use std::collections::BTreeMap;

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_unqualified_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{basename, get_command_name_and_token, get_unquoted_literal, only_literal_string};
use shellcheck_core::interface::TokenComment;

use crate::support::{analyse, functions, run_node_analysis};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkFunctionsUsedExternally",
    registry: "tree",
    order: 3,
    optional: None,
    kind: CheckKind::Tree(check_functions_used_externally),
};

// aliases t = Map.fromList $ analyse findAliases t
// `Map.fromList` over the reversed (prepended) list keeps the first
// alias in pre-order for each name.
fn aliases(ast: &shellcheck_core::ast::Ast, t: Id) -> BTreeMap<String, Id> {
    let mut map = BTreeMap::new();
    for (name, id) in analyse(ast, &mut |x, st| find_aliases(ast, x, st), t) {
        map.entry(name).or_insert(id);
    }
    map
}

fn find_aliases(ast: &shellcheck_core::ast::Ast, t: Id, st: &mut Vec<(String, Id)>) {
    if let Token::T_SimpleCommand(_, words) = ast.get(t)
        && words.len() >= 1
        && is_unqualified_command(ast, t, "alias")
    {
        for &arg in &words[1..] {
            get_alias(ast, arg, st);
        }
    }
}

fn get_alias(ast: &shellcheck_core::ast::Ast, arg: Id, st: &mut Vec<(String, Id)>) {
    let string = only_literal_string(ast, arg);
    if string.contains('=') {
        let name: String = string.chars().take_while(|&c| c != '=').collect();
        st.push((name, arg));
    }
}

fn check_functions_used_externally(p: &Params<'_>, t: Id) -> Vec<TokenComment> {
    let functions_and_aliases: BTreeMap<String, Id> = {
        let mut m = functions(p.ast, t);
        for (k, v) in aliases(p.ast, t) {
            m.entry(k).or_insert(v);
        }
        m
    };
    run_node_analysis(&|p, x, c| check_command(p, x, c, &functions_and_aliases), p, t)
}

fn check_command(p: &Params<'_>, t: Id, c: &mut Comments, fa: &BTreeMap<String, Id>) {
    let Token::T_SimpleCommand(_, argv) = p.ast.get(t) else { return };
    let (Some(str), t) = get_command_name_and_token(p.ast, false, t) else { return };
    let name = basename(&str);
    let args = skip_over(p, t, argv);
    let arg_strings: Vec<(String, Id)> = args.iter().map(|&x| (only_literal_string(p.ast, x), x)).collect();
    let candidates = get_potential_commands(&name, &arg_strings);
    for (_, arg) in candidates {
        check_arg(p, c, fa, &name, t, *arg);
    }
}

fn skip_over<'a>(p: &Params<'_>, t: Id, list: &'a [Id]) -> &'a [Id] {
    let _ = p;
    let rest: &[Id] = match list.iter().position(|&c| c == t) {
        Some(i) => &list[i..],
        None => &[],
    };
    if rest.is_empty() { rest } else { &rest[1..] }
}

/// Try to pick out the argument[s] that may be commands
fn get_potential_commands<'a>(name: &str, arg_and_string: &'a [(String, Id)]) -> Vec<&'a (String, Id)> {
    let drop_flags = || arg_and_string.iter().skip_while(|x| x.0.starts_with('-'));
    let first_non_flag = || drop_flags().take(1).collect::<Vec<_>>();
    match name {
        "chroot" | "screen" | "sudo" | "doas" | "run0" | "xargs" | "tmux" => first_non_flag(),
        "timeout" | "ssh" => drop_flags().skip(1).take(1).collect(),
        "find" => {
            let find_exec_flags = ["-exec", "-execdir", "-ok", "-okdir"];
            arg_and_string
                .iter()
                .skip_while(|x| !find_exec_flags.contains(&x.0.as_str()))
                .skip(1)
                .take(1)
                .collect()
        }
        _ => vec![],
    }
}

fn pattern_context(p: &Params<'_>, id: Id) -> String {
    // posLine . fst <$> Map.lookup id (tokenPositions params)
    let (start, _) = p.ast.span(id);
    format!(" on line {}.", start.line)
}

fn check_arg(p: &Params<'_>, c: &mut Comments, fa: &BTreeMap<String, Id>, cmd: &str, cmd_id: Id, arg: Id) {
    let Some(literal_arg) = get_unquoted_literal(p.ast, arg) else { return }; // only consider unquoted literals
    let Some(&definition_id) = fa.get(&literal_arg) else { return };
    c.warn(
        arg,
        2033,
        "Shell functions can't be passed to external commands. Use separate script or sh -c.",
    );
    c.info(
        definition_id,
        2032,
        format!("This function can't be invoked via {}{}", cmd, pattern_context(p, cmd_id)),
    );
}
