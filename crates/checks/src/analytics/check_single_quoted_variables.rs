// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkSingleQuotedVariables`, lines 1137-1232 (e11f964)

use std::sync::LazyLock;

use regex::Regex;
use shellcheck_analyzer::analyzerlib::{Comments, Params, get_closest_command};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::{basename, get_command_basename, get_literal_string, get_path};
use shellcheck_core::regex::{matches, mk_regex};

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkSingleQuotedVariables",
    registry: "node",
    order: 47,
    optional: None,
    kind: CheckKind::Node(check_single_quoted_variables),
};

static RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("\\$[{(0-9a-zA-Z_]|`[^`]+`"));
static SED_CONTRA: LazyLock<Regex> = LazyLock::new(|| {
    mk_regex(
        "\\$[acdgGilpPsxy]($|[^a-zA-Z])|\\$\\{[[:space:]]*($|/|[acdgGilpPsxy]($|[^a-zA-Z]))",
    )
});
static DASH_C_RE: LazyLock<Regex> = LazyLock::new(|| mk_regex("^-[a-zA-Z]*c[a-zA-Z]*$"));

fn check_single_quoted_variables(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SingleQuoted(s) = p.ast.get(t) else { return };
    let ast = p.ast;
    let command_name: String = (|| {
        let cmd = get_closest_command(ast, t)?;
        let name = get_command_basename(ast, cmd)?;
        Some(if name == "find" {
            get_find_command(ast, cmd)
        } else if name == "git" {
            get_git_command(ast, cmd)
        } else if name == "mumps" {
            get_mumps_command(ast, cmd)
        } else {
            name
        })
    })()
    .unwrap_or_default();

    if matches(s, &RE) {
        if command_name == "sed" {
            if !matches(s, &SED_CONTRA) {
                show_message(c, t);
            }
        } else if !is_probably_ok(ast, t, &command_name) {
            show_message(c, t);
        }
    }
}

fn show_message(c: &mut Comments, id: Id) {
    c.info(id, 2016, "Expressions don't expand in single quotes, use double quotes for that.");
}

fn is_probably_ok(ast: &Ast, t: Id, command_name: &str) -> bool {
    const OK_COMMANDS: &[&str] = &[
        "trap",
        "sh",
        "bash",
        "ksh",
        "zsh",
        "ssh",
        "eval",
        "xprop",
        "alias",
        "sudo",
        "doas",
        "run0",
        "docker",
        "podman",
        "oc",
        "dpkg-query",
        "jq",
        "rename",
        "rg",
        "unset",
        "git filter-branch",
        "mumps -run %XCMD",
        "mumps -run LOOP%XCMD",
    ];
    get_path(ast, t).iter().take(3).any(|&x| is_ok_assignment(ast, x))
        || OK_COMMANDS.contains(&command_name)
        || command_name.ends_with("awk")
        || command_name.starts_with("perl")
        || is_shell_script_argument(ast, t)
}

// Like `runc exec ctr sh -c 'echo $HOME'`
fn is_shell_script_argument(ast: &Ast, t: Id) -> bool {
    (|| {
        let cmd = get_closest_command(ast, t)?;
        let Token::T_Redirecting(_, inner) = ast.get(cmd) else { return None };
        let Token::T_SimpleCommand(_, words) = ast.get(*inner) else { return None };
        let path_ids = get_path(ast, t);
        let before: Vec<String> = words
            .iter()
            .take_while(|w| !path_ids.contains(w))
            .map(|&w| get_literal_string(ast, w).unwrap_or_default())
            .collect();
        Some((0..=before.len()).any(|i| is_shell_invocation(&before[i..])))
    })()
    .unwrap_or(false)
}

fn is_shell_invocation(l: &[String]) -> bool {
    let [cmd, opts @ ..] = l else { return false };
    [
        "sh", "bash", "dash", "ash", "ksh", "ksh88", "ksh93", "mksh", "oksh", "zsh",
    ]
    .contains(&basename(cmd).as_str())
        && ends_with_dash_c(opts)
}

fn ends_with_dash_c(opts: &[String]) -> bool {
    match opts {
        [flag] => matches(flag, &DASH_C_RE),
        [flag, _, rest @ ..] if ["-o", "+o", "-O", "+O"].contains(&flag.as_str()) => {
            ends_with_dash_c(rest)
        }
        [flag, rest @ ..] if flag.starts_with('-') || flag.starts_with('+') => {
            ends_with_dash_c(rest)
        }
        _ => false,
    }
}

fn is_ok_assignment(ast: &Ast, t: Id) -> bool {
    const COMMONLY_QUOTED: &[&str] = &["PS1", "PS2", "PS3", "PS4", "PROMPT_COMMAND"];
    match ast.get(t) {
        Token::T_Assignment(_, name, _, _) => COMMONLY_QUOTED.contains(&name.as_str()),
        Token::TC_Unary(_, op, _) if op == "-v" => true,
        _ => false,
    }
}

fn get_find_command(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_SimpleCommand(_, words) => {
            let list: Vec<Option<String>> =
                words.iter().map(|&w| get_literal_string(ast, w)).collect();
            let is_exec = |x: &Option<String>| {
                matches!(x.as_deref(), Some("-exec" | "-execdir" | "-ok" | "-okdir"))
            };
            let cmd: Vec<&Option<String>> = list.iter().skip_while(|x| !is_exec(x)).collect();
            match cmd.as_slice() {
                [_, cmd, ..] => (*cmd).clone().unwrap_or_else(|| "find".to_string()),
                _ => "find".to_string(),
            }
        }
        Token::T_Redirecting(_, cmd) => get_find_command(ast, *cmd),
        _ => "find".to_string(),
    }
}

fn get_git_command(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_SimpleCommand(_, words) => {
            let l: Vec<Option<String>> =
                words.iter().map(|&w| get_literal_string(ast, w)).collect();
            match l.as_slice() {
                [Some(a), Some(b), ..] if a == "git" && b == "filter-branch" => {
                    "git filter-branch".to_string()
                }
                _ => "git".to_string(),
            }
        }
        Token::T_Redirecting(_, cmd) => get_git_command(ast, *cmd),
        _ => "git".to_string(),
    }
}

fn get_mumps_command(ast: &Ast, t: Id) -> String {
    match ast.get(t) {
        Token::T_SimpleCommand(_, words) => {
            let l: Vec<Option<String>> =
                words.iter().map(|&w| get_literal_string(ast, w)).collect();
            match l.as_slice() {
                [Some(a), Some(b), Some(c), ..] if a == "mumps" && b == "-run" && c == "%XCMD" => {
                    "mumps -run %XCMD".to_string()
                }
                [Some(a), Some(b), Some(c), ..]
                    if a == "mumps" && b == "-run" && c == "LOOP%XCMD" =>
                {
                    "mumps -run LOOP%XCMD".to_string()
                }
                _ => "mumps".to_string(),
            }
        }
        Token::T_Redirecting(_, cmd) => get_mumps_command(ast, *cmd),
        _ => "mumps".to_string(),
    }
}
