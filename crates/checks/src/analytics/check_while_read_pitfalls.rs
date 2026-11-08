// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkWhileReadPitfalls`, lines 2777-2846 (e11f964)

use crate::support::{fix_with, replace_end};
use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{
    get_all_flags, get_command_argv, get_command_basename, get_command_sequences,
    get_command_token_or_this, get_literal_string, get_word_parts, oversimplify,
};
use shelllint_core::prelude::head_or_default;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkWhileReadPitfalls",
    registry: "node",
    order: 43,
    optional: None,
    kind: CheckKind::Node(check_while_read_pitfalls),
};

#[derive(Clone, Copy)]
enum Check {
    HasFlag,
    HasArgument,
    Never,
}

#[derive(Clone, Copy)]
enum Fix {
    AddFlag,
    AddRedirect,
}

fn check_while_read_pitfalls(p: &Params<'_>, id: Id, c: &mut Comments) {
    let Token::T_WhileExpression(conds, contents) = p.ast.get(id) else { return };
    let &[command] = &conds[..] else { return };
    if is_stdin_read_command(p, command) {
        for &x in contents {
            check_muncher(p, id, x, c);
        }
    }
}

// Map of munching commands to a function that checks if the flags should exclude it
fn munchers(name: &str) -> Option<(Check, Fix, &'static str)> {
    match name {
        "ssh" => Some((Check::HasFlag, Fix::AddFlag, "-n")),
        "ffmpeg" => Some((Check::HasArgument, Fix::AddFlag, "-nostdin")),
        "mplayer" => Some((Check::HasArgument, Fix::AddFlag, "-noconsolecontrols")),
        "HandBrakeCLI" => Some((Check::Never, Fix::AddRedirect, "< /dev/null")),
        _ => None,
    }
}

// Use flag parsing, e.g. "-an" -> "a", "n"
fn has_flag(p: &Params<'_>, arg: &str, cmd: Id) -> bool {
    // Haskell pattern ('-':flag) is partial; every muncher flag starts with '-'
    let Some(flag) = arg.strip_prefix('-') else { panic!("hasFlag: non-exhaustive pattern") };
    get_all_flags(p.ast, cmd).iter().any(|(_, f)| f == flag)
}

// Simple string match, e.g. "-an" -> "-an"
fn has_argument(p: &Params<'_>, arg: &str, cmd: Id) -> bool {
    let argv = get_command_argv(p.ast, cmd).expect("fromJust: Nothing");
    argv.iter().filter_map(|&a| get_literal_string(p.ast, a)).any(|s| s == arg)
}

fn run_check(p: &Params<'_>, k: Check, flag: &str, cmd: Id) -> bool {
    match k {
        Check::HasFlag => has_flag(p, flag, cmd),
        Check::HasArgument => has_argument(p, flag, cmd),
        Check::Never => false,
    }
}

fn make_fix(p: &Params<'_>, k: Fix, string: &str, cmd: Id) -> shelllint_core::interface::Fix {
    let target = match k {
        Fix::AddFlag => get_command_token_or_this(p.ast, cmd),
        Fix::AddRedirect => cmd,
    };
    fix_with(vec![replace_end(target, p, 0, &format!(" {}", string))])
}

fn is_stdin_read_command(p: &Params<'_>, t: Id) -> bool {
    let Token::T_Pipeline(_, cmds) = p.ast.get(t) else { return false };
    let &[r] = &cmds[..] else { return false };
    let Token::T_Redirecting(redirs, cmd) = p.ast.get(r) else { return false };
    let plaintext = oversimplify(p.ast, *cmd);
    head_or_default(String::new(), &plaintext) == "read"
        && !plaintext.iter().any(|s| s == "-u")
        && !redirs.iter().any(|&r| stdin_redirect(p, r))
}

fn check_muncher(p: &Params<'_>, id: Id, t: Id, c: &mut Comments) {
    match p.ast.get(t) {
        Token::T_Pipeline(_, cmds) => {
            let Some(&first) = cmds.first() else { return };
            let Token::T_Redirecting(redirs, cmd) = p.ast.get(first) else { return };
            let cmd = *cmd;
            // Check command substitutions regardless of the command
            if let Token::T_SimpleCommand(vars, args) = p.ast.get(cmd) {
                for &w in vars.iter().chain(args.iter()) {
                    for part in get_words(p, w) {
                        for seq in get_command_sequences(p.ast, part) {
                            for x in seq {
                                check_muncher(p, id, x, c);
                            }
                        }
                    }
                }
            }

            if !redirs.iter().any(|&r| stdin_redirect(p, r)) {
                // Recurse into ifs/loops/groups/etc if this doesn't redirect
                for seq in get_command_sequences(p.ast, cmd) {
                    for x in seq {
                        check_muncher(p, id, x, c);
                    }
                }

                // Check the actual command
                let Some(name) = get_command_basename(p.ast, cmd) else { return };
                let Some((check, fix, flag)) = munchers(&name) else { return };
                if run_check(p, check, flag, cmd) {
                    return;
                }
                c.info(id, 2095, format!("{} may swallow stdin, preventing this loop from working properly.", name));
                c.warn_with_fix(
                    cmd,
                    2095,
                    format!("Use {} {} to prevent {} from swallowing stdin.", name, flag, name),
                    make_fix(p, fix, flag, cmd),
                );
            }
        }
        Token::T_Backgrounded(inner) => check_muncher(p, id, *inner, c),
        _ => {}
    }
}

fn stdin_redirect(p: &Params<'_>, t: Id) -> bool {
    let Token::T_FdRedirect(fd, op) = p.ast.get(t) else { return false };
    if fd == "0" {
        true
    } else if fd.is_empty() {
        match p.ast.get(*op) {
            Token::T_IoFile(o, _) => matches!(p.ast.get(*o), Token::T_Less),
            Token::T_IoDuplicate(o, _) => matches!(p.ast.get(*o), Token::T_LESSAND),
            Token::T_HereString(_) => true,
            Token::T_HereDoc(..) => true,
            _ => false,
        }
    } else {
        false
    }
}

fn get_words(p: &Params<'_>, t: Id) -> Vec<Id> {
    match p.ast.get(t) {
        Token::T_Assignment(_, _, _, x) => get_word_parts(p.ast, *x),
        _ => get_word_parts(p.ast, t),
    }
}
