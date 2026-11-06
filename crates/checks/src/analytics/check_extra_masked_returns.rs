// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkExtraMaskedReturns`, lines 5125-5201 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, parents};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::{
    basename, get_all_flags, get_bsd_opts, get_command, get_command_basename, get_literal_string, is_flag,
};
use shellcheck_core::data::DECLARING_COMMANDS;
use shellcheck_core::interface::TokenComment;

use crate::support::run_node_analysis;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkExtraMaskedReturns",
    registry: "optional-tree",
    order: 9,
    optional: Some(StaticCheckDescription {
        name: "check-extra-masked-returns",
        id: &[2312],
        description: "Check for additional cases where exit codes are masked",
        positive: "rm -r \"$(get_chroot_dir)/home\"",
        negative: "set -e; dir=\"$(get_chroot_dir)\"; rm -r \"$dir/home\"",
    }),
    kind: CheckKind::Tree(check_extra_masked_returns),
};

fn check_extra_masked_returns(params: &Params<'_>, t: Id) -> Vec<TokenComment> {
    // `removeTransparentCommands` is not materialised: each check below
    // reads a `T_SimpleCommand`'s words through `effective_words`, which is
    // what the rewritten token would hold. Ancestor lookups
    // (`parents params`) use the original tree, as the Haskell params do.
    // The walks still visit a dropped `time` word, which is harmless: a word
    // whose command basename is literally `time` holds no command.
    let sast: &Ast = params.ast;

    let find_masking_nodes = |_: &Params<'_>, id: Id, c: &mut Comments| {
        let find_in_list = |l: &[Id], c: &mut Comments| {
            for &n in l {
                do_analysis_find_masked(params, sast, n, c);
            }
        };
        match sast.get(id) {
            Token::T_Arithmetic(list) => find_in_list(&[*list], c),
            Token::T_Array(list) => find_in_list(&all_but_last_simple_commands(sast, list), c),
            Token::T_Condition(_, _, condition) => find_in_list(&[*condition], c),
            Token::T_DoubleQuoted(list) => find_in_list(&all_but_last_simple_commands(sast, list), c),
            Token::T_HereDoc(_, _, _, list) => find_in_list(list, c),
            Token::T_HereString(word) => find_in_list(&[*word], c),
            Token::T_NormalWord(parts) => find_in_list(&all_but_last_simple_commands(sast, parts), c),
            Token::T_Pipeline(_, cmds) if !params.has_pipefail => {
                find_in_list(&all_but_last_simple_commands(sast, cmds), c)
            }
            Token::T_ProcSub(_, list) => find_in_list(list, c),
            Token::T_SimpleCommand(assigns, _) => match effective_words(sast, id) {
                [_, rest @ ..] => {
                    let mut l = assigns.clone();
                    l.extend_from_slice(rest);
                    find_in_list(&l, c)
                }
                [] => find_in_list(&all_but_last_simple_commands(sast, assigns), c),
            },
            _ => {}
        }
    };
    run_node_analysis(&find_masking_nodes, params, t)
}

/// `doAnalysis findMaskedNodes n`
fn do_analysis_find_masked(params: &Params<'_>, sast: &Ast, n: Id, c: &mut Comments) {
    sast.analyze(n, &mut |t| find_masked_nodes(params, sast, t, c), &mut |_| {});
}

fn find_masked_nodes(params: &Params<'_>, sast: &Ast, t: Id, c: &mut Comments) {
    match sast.get(t) {
        Token::T_SimpleCommand(..) if !effective_words(sast, t).is_empty() => {
            if is_masked_node(params, sast, t) {
                inform(t, c);
            }
        }
        Token::T_Condition(..) => {
            if is_masked_node(params, sast, t) {
                inform(t, c);
            }
        }
        _ => {}
    }
}

fn is_masked_node(params: &Params<'_>, sast: &Ast, t: Id) -> bool {
    !(is_harmless_command(sast, t) || is_checked_elsewhere(params, t) || is_mask_deliberate(params, t))
}

/// `containsSimpleCommand t = isNothing $ doAnalysis go t`
// `doAnalysis` in `Maybe` stops at the first `T_SimpleCommand`; walking the
// whole subtree instead is quadratic in the nesting depth.
fn contains_simple_command(ast: &Ast, t: Id) -> bool {
    let mut stack = vec![t];
    while let Some(x) = stack.pop() {
        if matches!(ast.get(x), Token::T_SimpleCommand(..)) {
            return true;
        }
        stack.extend(ast.children(x));
    }
    false
}

fn all_but_last_simple_commands(ast: &Ast, cmds: &[Id]) -> Vec<Id> {
    let simple: Vec<Id> = cmds.iter().copied().filter(|&c| contains_simple_command(ast, c)).collect();
    if simple.is_empty() { vec![] } else { simple[..simple.len() - 1].to_vec() }
}

/// The words of `T_SimpleCommand` `t` after `removeTransparentCommands`:
/// ```haskell
/// removeTransparentCommands t =
///     doTransform go t
///   where
///     go cmd@(T_SimpleCommand id assigns (_:t)) | isTransparentCommand cmd = T_SimpleCommand id assigns t
///     go t = t
/// ```
// The words themselves are never rewritten, so the transform's bottom-up
// order does not change which commands are transparent.
fn effective_words(ast: &Ast, t: Id) -> &[Id] {
    let Token::T_SimpleCommand(_, words) = ast.get(t) else { return &[] };
    match &words[..] {
        [_, rest @ ..] if is_transparent_command(ast, t) => rest,
        _ => words,
    }
}

fn inform(t: Id, c: &mut Comments) {
    c.info(
        t,
        2312,
        "Consider invoking this command separately to avoid masking its return value (or use '|| true' to ignore).",
    );
}

fn is_mask_deliberate(params: &Params<'_>, t: Id) -> bool {
    let ps = parents(params, t);
    // NE.init: all but the last (the root)
    ps[..ps.len() - 1].iter().any(|&x| is_or_if(params.ast, x))
}

fn is_or_if(ast: &Ast, t: Id) -> bool {
    let Token::T_OrIf(_, rhs) = ast.get(t) else { return false };
    let Token::T_Pipeline(_, cmds) = ast.get(*rhs) else { return false };
    let [only] = &cmds[..] else { return false };
    let Token::T_Redirecting(_, cmd) = ast.get(*only) else { return false };
    matches!(get_command_basename(ast, *cmd).as_deref(), Some("true" | ":" | "echo"))
}

fn is_checked_elsewhere(params: &Params<'_>, t: Id) -> bool {
    let ps = parents(params, t);
    // NE.tail: all but the first (the node itself)
    ps[1..].iter().any(|&x| is_declaring_command(params.ast, x))
}

fn is_declaring_command(ast: &Ast, t: Id) -> bool {
    let Some(cmd) = get_command(ast, t) else { return false };
    let Some(basename) = get_command_basename(ast, cmd) else { return false };
    match basename.as_str() {
        // local -r x=$(false) is intentionally ignored for SC2155
        "local" if get_all_flags(ast, cmd).iter().any(|(_, f)| f == "r") => false,
        b => DECLARING_COMMANDS.contains(&b),
    }
}

fn is_harmless_command(ast: &Ast, t: Id) -> bool {
    match command_name_of_words(ast, effective_words(ast, t)).map(|s| basename(&s)) {
        Some(b) => matches!(b.as_str(), "echo" | "basename" | "dirname" | "printf" | "set" | "shopt"),
        None => false,
    }
}

fn is_transparent_command(ast: &Ast, t: Id) -> bool {
    get_command_basename(ast, t).as_deref() == Some("time")
}

/// `getCommandName` of a `T_SimpleCommand` holding `words`
/// (`astlib::get_command_name_and_token False`).
fn command_name_of_words(ast: &Ast, words: &[Id]) -> Option<String> {
    let (&w, rest) = words.split_first()?;
    let s = get_literal_string(ast, w)?;
    let first_arg = || {
        let arg = *rest.first()?;
        if is_flag(ast, arg) { None } else { Some(arg) }
    };
    let actual = match s.as_str() {
        "busybox" | "builtin" | "command" | "run" => first_arg(),
        "exec" => get_bsd_opts(ast, "cla:", rest)
            .and_then(|opts| opts.into_iter().find(|(name, _)| name.is_empty()))
            .map(|(_, (t, _))| t),
        _ => None,
    };
    match actual {
        Some(actual) => get_literal_string(ast, actual),
        None => Some(s),
    }
}
