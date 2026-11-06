// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPipeToNowhere`, lines 3856-4022 (e11f964)

use std::collections::BTreeMap;

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::astlib::{get_all_flags, get_command, get_command_basename, has_flag};
use shellcheck_core::data::NON_READING_COMMANDS;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPipeToNowhere",
    registry: "node",
    order: 78,
    optional: None,
    kind: CheckKind::Node(check_pipe_to_nowhere),
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PipeType {
    StdoutPipe,
    StdoutStderrPipe,
    NoPipe,
}

const INTERACTIVE_FLAG_CMDS: &[&str] = &["cp", "mv", "rm"];

fn check_pipe_to_nowhere(p: &Params<'_>, t: Id, c: &mut Comments) {
    let ast = p.ast;
    match ast.get(t) {
        Token::T_Pipeline(pipes, cmds) => {
            for (input, stage, output) in commands_with_context(ast, pipes, cmds) {
                check_pipe(ast, input, stage, output, c);
            }
        }
        Token::T_Redirecting(redirects, cmd) if redirects.iter().any(|&r| redirects_stdin(ast, r)) => {
            check_redir(ast, *cmd, c)
        }
        _ => {}
    }
}

fn check_pipe(ast: &Ast, input: PipeType, stage: Id, output: PipeType, c: &mut Comments) {
    // Haskell's `let` bindings are lazy: each subtree scan runs only if a
    // guard reaches it. Scanning eagerly at every stage is quadratic in the
    // nesting depth (e.g. deeply nested `$(...)`).
    let has_consumers = std::cell::OnceCell::new();
    let has_consumers = || *has_consumers.get_or_init(|| has_additional_consumers(ast, stage));
    let has_producers = std::cell::OnceCell::new();
    let has_producers = || *has_producers.get_or_init(|| has_additional_producers(ast, stage));

    (|| {
        let cmd = get_command(ast, stage)?;
        let name = get_command_basename(ast, cmd)?;
        if !NON_READING_COMMANDS.contains(&name.as_str()) {
            return None;
        }
        if !(!has_consumers() && input != PipeType::NoPipe) {
            return None;
        }
        if command_specific_exception(ast, &name, cmd) {
            return None;
        }

        // Confusing echo for cat is so common that it's worth a special case
        let suggestion = if name == "echo" {
            "Did you want 'cat' instead?"
        } else {
            "Wrong command or missing xargs?"
        };
        c.warn(
            cmd,
            2216,
            format!("Piping to '{}', a command that doesn't read stdin. {}", name, suggestion),
        );
        Some(())
    })();

    (|| {
        let Token::T_Redirecting(redirs, _cmd) = ast.get(stage) else { return None };
        let mut fds = Vec::new();
        for &r in redirs {
            fds.push(get_redirection_fds(ast, r)?);
        }

        let mut fd_map: BTreeMap<u128, Vec<Id>> = BTreeMap::new();
        for (list, &redir) in fds.iter().zip(redirs.iter()) {
            for &n in list {
                // Map.fromListWith (++): new elements are prepended
                fd_map.entry(n).or_default().insert(0, redir);
            }
        }

        let input_warning = |c: &mut Comments| {
            if input != PipeType::NoPipe && !has_consumers() {
                if let Some(&over) = fd_map.get(&0).and_then(|l| l.first()) {
                    c.err(
                        get_op_id(ast, over),
                        2259,
                        "This redirection overrides piped input. To use both, merge or pass filenames.",
                    );
                }
            }
        };

        // Only produce output warnings for regular pipes, since these are
        // way more common, and  `foo > out 2> err |& foo` can still write
        // to stderr if the files fail to open
        let output_warning = |c: &mut Comments| {
            if output == PipeType::StdoutPipe && !has_producers() {
                if let Some(&over) = fd_map.get(&1).and_then(|l| l.first()) {
                    c.err(
                        get_op_id(ast, over),
                        2260,
                        "This redirection overrides the output pipe. Use 'tee' to output to both.",
                    );
                }
            }
        };

        input_warning(c);
        output_warning(c);
        for (n, list) in &fd_map {
            warn_about_dupes(ast, *n, list, c);
        }
        Some(())
    })();
}

fn command_specific_exception(ast: &Ast, name: &str, cmd: Id) -> bool {
    match name {
        "du" => get_all_flags(ast, cmd)
            .iter()
            .any(|(_, s)| s == "exclude-from" || s == "files0-from"),
        _ if INTERACTIVE_FLAG_CMDS.contains(&name) => has_interactive_flag(ast, cmd),
        _ => false,
    }
}

fn warn_about_dupes(ast: &Ast, n: u128, list: &[Id], c: &mut Comments) {
    if list.len() >= 2 {
        for &r in list {
            c.err(
                get_op_id(ast, r),
                2261,
                format!(
                    "Multiple redirections compete for {}. Use cat, tee, or pass filenames instead.",
                    fd_str(n)
                ),
            );
        }
    }
}

// DIVERGENCE: Haskell's local `alternative` is unused, so it is not ported.

fn fd_str(n: u128) -> String {
    match n {
        0 => "stdin".to_string(),
        1 => "stdout".to_string(),
        2 => "stderr".to_string(),
        _ => format!("FD {}", n),
    }
}

fn check_redir(ast: &Ast, cmd: Id, c: &mut Comments) {
    (|| {
        let name = get_command_basename(ast, cmd)?;
        if !NON_READING_COMMANDS.contains(&name.as_str()) {
            return None;
        }
        if has_additional_consumers(ast, cmd) {
            return None;
        }
        if INTERACTIVE_FLAG_CMDS.contains(&name.as_str()) && has_interactive_flag(ast, cmd) {
            return None;
        }
        let suggestion = if name == "echo" {
            "Did you want 'cat' instead?"
        } else {
            "Bad quoting, wrong command or missing xargs?"
        };
        c.warn(
            cmd,
            2217,
            format!("Redirecting to '{}', a command that doesn't read stdin. {}", name, suggestion),
        );
        Some(())
    })();
}

/// Could any words in a SimpleCommand consume stdin (e.g. echo "$(cat)")?
fn has_additional_consumers(ast: &Ast, t: Id) -> bool {
    tree_contains(ast, t, &|x| may_consume(ast, x))
}

/// Could any words in a SimpleCommand produce stdout? E.g. >(tee foo)
fn has_additional_producers(ast: &Ast, t: Id) -> bool {
    tree_contains(ast, t, &|x| may_produce(ast, x))
}

/// `isNothing $ doAnalysis (guard . not . pred) t`
fn tree_contains(ast: &Ast, root: Id, pred: &dyn Fn(Id) -> bool) -> bool {
    let mut stack = vec![root];
    while let Some(t) = stack.pop() {
        if pred(t) {
            return true;
        }
        stack.extend(ast.children(t));
    }
    false
}

fn has_interactive_flag(ast: &Ast, cmd: Id) -> bool {
    has_flag(ast, cmd, "i") || has_flag(ast, cmd, "interactive")
}

fn may_consume(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_ProcSub(dir, _) if dir == "<" => true,
        Token::T_Backticked(..) => true,
        Token::T_DollarExpansion(..) => true,
        _ => false,
    }
}

fn may_produce(ast: &Ast, t: Id) -> bool {
    matches!(ast.get(t), Token::T_ProcSub(dir, _) if dir == ">")
}

fn get_op_id(ast: &Ast, t: Id) -> Id {
    match ast.get(t) {
        Token::T_FdRedirect(_, x) => get_op_id(ast, *x),
        Token::T_IoFile(op, _) => *op,
        _ => t,
    }
}

fn get_redirection_fds(ast: &Ast, t: Id) -> Option<Vec<u128>> {
    match ast.get(t) {
        Token::T_FdRedirect(num, x) => {
            if num.is_empty() {
                get_default_fds(ast, *x)
            } else if num == "&" {
                Some(vec![1, 2])
            } else if num.chars().all(|ch| ch.is_ascii_digit()) {
                // Don't report the number unless we know what it is.
                // This avoids triggering on 3>&1 1>&3
                get_default_fds(ast, *x)?;
                // DIVERGENCE: an unparseable (overflowing) number yields Nothing
                num.parse::<u128>().ok().map(|n| vec![n])
            } else {
                // Don't bother with {fd}>42 and such
                None
            }
        }
        _ => None,
    }
}

fn get_default_fds(ast: &Ast, redir: Id) -> Option<Vec<u128>> {
    match ast.get(redir) {
        Token::T_HereDoc(..) => Some(vec![0]),
        Token::T_HereString(..) => Some(vec![0]),
        Token::T_IoFile(op, _) => match ast.get(*op) {
            Token::T_Less => Some(vec![0]),
            Token::T_Greater => Some(vec![1]),
            Token::T_DGREAT => Some(vec![1]),
            Token::T_GREATAND => Some(vec![1, 2]),
            Token::T_CLOBBER => Some(vec![1]),
            Token::T_IoDuplicate(op2, s) if s == "-" => get_default_fds(ast, *op2),
            _ => None,
        },
        _ => None,
    }
}

fn redirects_stdin(ast: &Ast, t: Id) -> bool {
    get_redirection_fds(ast, t).is_some_and(|fds| fds.contains(&0))
}

fn pipe_type(ast: &Ast, t: Id) -> PipeType {
    match ast.get(t) {
        Token::T_Pipe(s) if s == "|" => PipeType::StdoutPipe,
        Token::T_Pipe(s) if s == "|&" => PipeType::StdoutStderrPipe,
        _ => PipeType::NoPipe,
    }
}

fn commands_with_context(ast: &Ast, pipes: &[Id], cmds: &[Id]) -> Vec<(PipeType, Id, PipeType)> {
    let pipe_types: Vec<PipeType> = pipes.iter().map(|&p| pipe_type(ast, p)).collect();
    let inputs = std::iter::once(PipeType::NoPipe).chain(pipe_types.iter().copied());
    let outputs = pipe_types.iter().copied().chain(std::iter::once(PipeType::NoPipe));
    inputs
        .zip(cmds.iter().copied())
        .zip(outputs)
        .map(|((i, c), o)| (i, c, o))
        .collect()
}
