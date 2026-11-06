// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUncheckedCdPushdPopd`, lines 3421-3456 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_all_flags, get_command_name, get_literal_string, get_path, oversimplify};
use shellcheck_core::interface::TokenComment;
use shellcheck_core::regex::{matches, mk_regex};

use crate::support::{fix_with, is_condition, replace_end};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUncheckedCdPushdPopd",
    registry: "tree",
    order: 9,
    optional: None,
    kind: CheckKind::Tree(check_unchecked_cd_pushd_popd),
};

static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^/*((\\.|\\.\\.)/+)*(\\.|\\.\\.)?$"));

fn check_unchecked_cd_pushd_popd(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    if params.has_errexit {
        return vec![];
    }
    let mut c = Comments::new();
    // doAnalysis: pre-order traversal.
    params.ast.analyze(root, &mut |id| check_element(params, id, &mut c), &mut |_| {});
    c.into_vec()
}

fn check_element(params: &Params<'_>, t: Id, c: &mut Comments) {
    if !matches!(params.ast.get(t), Token::T_SimpleCommand(..)) {
        return;
    }
    let name = get_name(params, t);
    if ["cd", "pushd", "popd"].contains(&name.as_str())
        && !is_safe_dir(params, t)
        && !(["pushd", "popd"].contains(&name.as_str()) && get_all_flags(params.ast, t).iter().any(|(_, s)| s == "n"))
        && !is_last_command_in_function(params, t)
        && !is_condition(params.ast, &get_path(params.ast, t))
    {
        c.warn_with_fix(
            t,
            2164,
            format!(
                "Use '{name} ... || exit' or '{name} ... || return' in case {name} fails."
            ),
            fix_with(vec![replace_end(t, params, 0, " || exit")]),
        );
    }
}

fn get_name(params: &Params<'_>, t: Id) -> String {
    get_command_name(params.ast, t).unwrap_or_default()
}

fn is_last_command_in_function(params: &Params<'_>, t: Id) -> bool {
    let path = get_path(params.ast, t);
    let path = &path[1..];
    let ast = params.ast;
    let mut i = 0;
    while i < path.len() {
        let child = path[i];
        if let (Some(&bg), Some(&func)) = (path.get(i + 1), path.get(i + 2))
            && let Token::T_BraceGroup(commands) = ast.get(bg)
            && matches!(ast.get(func), Token::T_Function(..))
        {
            let rest: Vec<Id> = commands.iter().copied().skip_while(|x| *x != child).collect();
            return match &rest[..] {
                [_] => true,
                [_, last] => is_return_or_exit(params, *last),
                _ => false,
            };
        }
        i += 1;
    }
    false
}

fn is_return_or_exit(params: &Params<'_>, pipe: Id) -> bool {
    let ast = params.ast;
    let Token::T_Pipeline(a, b) = ast.get(pipe) else { return false };
    if !a.is_empty() {
        return false;
    }
    let &[redir] = &b[..] else { return false };
    let Token::T_Redirecting(r, cmd) = ast.get(redir) else { return false };
    if !r.is_empty() {
        return false;
    }
    let Token::T_SimpleCommand(assigns, words) = ast.get(*cmd) else { return false };
    if !assigns.is_empty() {
        return false;
    }
    let Some((&cmd, args)) = words.split_first() else { return false };
    let l = get_literal_string(ast, cmd);
    if !(l.as_deref() == Some("return") || l.as_deref() == Some("exit")) {
        return false;
    }
    match args {
        [] => true,
        [w] => {
            if let Token::T_NormalWord(parts) = ast.get(*w)
                && let &[d] = &parts[..]
                && let Token::T_DollarBraced(false, status) = ast.get(d)
            {
                get_literal_string(ast, *status).as_deref() == Some("?")
            } else {
                false
            }
        }
        _ => false,
    }
}

fn is_safe_dir(params: &Params<'_>, t: Id) -> bool {
    match &oversimplify(params.ast, t)[..] {
        [_, s] => matches(s, &RE),
        _ => false,
    }
}
