// SPDX-License-Identifier: GPL-3.0-or-later
// shelllint extension (issue koalaman/shellcheck#2574): no Haskell counterpart.
//
// DIVERGENCE: an optional check that upstream does not have. Unlike every
// upstream check its result depends on the machine: it looks at the `$PATH`
// of the process running the linter.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use shelllint_analyzer::analyzerlib::{Params, make_comment};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_command_name, get_command_name_and_token, get_literal_string};
use shelllint_core::interface::{Severity, TokenComment};

use crate::support::{analyse, functions};

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUndefinedCommand",
    registry: "optional-tree",
    order: 100,
    optional: Some(StaticCheckDescription {
        name: "undefined-command",
        id: &[9004],
        description: "Flag commands that are not builtins, functions or found in $PATH (result depends on the machine running the linter)",
        positive: "shelllint_no_such_command_xyz --flag",
        negative: "echo hello",
    }),
    kind: CheckKind::Tree(check_undefined_command),
};

/// Shell builtins and reserved words of bash, dash, ksh and zsh (the union:
/// being lenient about the shell avoids false positives).
const BUILTINS: &[&str] = &[
    ".", ":", "[", "!", "{", "}", "[[", "]]", "alias", "autoload", "bg", "bind", "break", "builtin", "caller", "case",
    "cd", "chdir", "command", "compgen", "complete", "compopt", "continue", "coproc", "declare", "dirs", "disown",
    "do", "done", "echo", "elif", "else", "enable", "esac", "eval", "exec", "exit", "export", "false", "fc", "fg",
    "fi", "float", "for", "function", "getopts", "hash", "help", "history", "if", "in", "integer", "jobs", "kill",
    "let", "local", "logout", "mapfile", "popd", "print", "printf", "pushd", "pwd", "read", "readarray", "readonly",
    "return", "select", "set", "shift", "shopt", "source", "suspend", "test", "then", "time", "times", "trap",
    "true", "type", "typeset", "ulimit", "umask", "unalias", "unset", "until", "wait", "whence", "while",
];

thread_local! {
    /// `$PATH` as last seen, with the answer of every lookup made under it.
    static PATH_CACHE: RefCell<Option<(String, HashMap<String, bool>)>> = const { RefCell::new(None) };
}

fn is_executable_file(p: &std::path::Path) -> bool {
    let Ok(md) = std::fs::metadata(p) else { return false };
    if !md.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        md.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Is `name` an executable file in one of the directories of `path`? Each
/// answer is cached for as long as `path` stays the same, so a name costs at
/// most one `stat` per `$PATH` entry, once per run.
fn found_on_path(path: &str, name: &str) -> bool {
    PATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|(p, _)| p != path) {
            *cache = Some((path.to_string(), HashMap::new()));
        }
        let (_, map) = cache.as_mut().unwrap();
        if let Some(&found) = map.get(name) {
            return found;
        }
        let found = std::env::split_paths(path).any(|dir| {
            // An empty `$PATH` entry means the current directory.
            let dir = if dir.as_os_str().is_empty() { std::path::PathBuf::from(".") } else { dir };
            is_executable_file(&dir.join(name))
        });
        map.insert(name.to_string(), found);
        found
    })
}

fn check_undefined_command(p: &Params<'_>, root: Id) -> Vec<TokenComment> {
    // Without a `$PATH` there is nothing to compare with; say nothing.
    let Some(path) = std::env::var_os("PATH").and_then(|s| s.into_string().ok()) else {
        return vec![];
    };
    let ast = p.ast;
    let defined: HashSet<String> = functions(ast, root).into_keys().collect();
    // `alias ll='ls -l'` defines a command name too.
    let aliases: HashSet<String> = analyse(ast, &mut |x, st: &mut Vec<String>| {
        if let Token::T_SimpleCommand(_, words) = ast.get(x) {
            if get_command_name(ast, x).as_deref() == Some("alias") {
                for &w in words.iter().skip(1) {
                    if let Some(s) = get_literal_string(ast, w) {
                        if let Some((name, _)) = s.split_once('=') {
                            st.push(name.to_string());
                        }
                    }
                }
            }
        }
    }, root)
    .into_iter()
    .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut comments = vec![];
    let cmds = analyse(ast, &mut |x, st: &mut Vec<Id>| {
        if matches!(ast.get(x), Token::T_SimpleCommand(..)) {
            st.push(x);
        }
    }, root);
    for cmd in cmds {
        let (Some(name), tok) = get_command_name_and_token(ast, false, cmd) else { continue };
        if name.is_empty() || name.contains('/') || name.starts_with('~') {
            continue;
        }
        if BUILTINS.contains(&name.as_str()) || defined.contains(&name) || aliases.contains(&name) {
            continue;
        }
        if !seen.insert(name.clone()) || found_on_path(&path, &name) {
            continue;
        }
        comments.push(make_comment(
            Severity::InfoC,
            tok,
            9004,
            format!("'{name}' is not a shell builtin, not defined in this script and not found in $PATH on this machine."),
        ));
    }
    comments
}
