// SPDX-License-Identifier: GPL-3.0-or-later
// Not in ShellCheck: shelllint's own optional check `require-strict-mode` (SC9001, issue #409)

use shelllint_analyzer::analyzerlib::{Comments, Params, is_unqualified_command};
use shelllint_analyzer::api::{CheckEntry, CheckKind, StaticCheckDescription};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::oversimplify;
use shelllint_core::interface::{Shell, TokenComment};

// DIVERGENCE: shelllint-only check, so there is no Haskell definition or order;
// the order only has to be unique among the optional-tree entries.
pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRequireStrictMode",
    registry: "optional-tree",
    order: 11,
    optional: Some(StaticCheckDescription {
        name: "require-strict-mode",
        id: &[9001],
        description: "Notify when a script does not set errexit, nounset and pipefail",
        positive: "echo hi",
        negative: "set -euo pipefail; echo hi",
    }),
    kind: CheckKind::Tree(check_require_strict_mode),
};

#[derive(Default)]
struct Seen {
    errexit: bool,
    nounset: bool,
    pipefail: bool,
}

impl Seen {
    /// Record the options enabled by a list of `set`-style arguments such as
    /// `-euo pipefail` or `-o errexit -o nounset`. `+x` words turn options
    /// off and are ignored, like upstream's "mentioned anywhere" heuristics.
    fn args<'a>(&mut self, words: impl Iterator<Item = &'a str>) {
        let mut want_name = false;
        for w in words {
            if want_name {
                want_name = false;
                self.named(w);
                continue;
            }
            if w == "--" {
                break;
            }
            if let Some(flags) = w.strip_prefix('-') {
                for f in flags.chars() {
                    match f {
                        'e' => self.errexit = true,
                        'u' => self.nounset = true,
                        'o' => want_name = true,
                        _ => {}
                    }
                }
            }
        }
    }

    fn named(&mut self, name: &str) {
        match name {
            "errexit" => self.errexit = true,
            "nounset" => self.nounset = true,
            "pipefail" => self.pipefail = true,
            _ => {}
        }
    }
}

fn check_require_strict_mode(params: &Params<'_>, root: Id) -> Vec<TokenComment> {
    let ast = params.ast;
    let mut seen = Seen::default();

    // The shebang, e.g. `#!/bin/bash -eu` or `#!/usr/bin/env -S bash -euo pipefail`
    let mut script = root;
    while let Token::T_Annotation(_, inner) = ast.get(script) {
        script = *inner;
    }
    let shebang = match ast.get(script) {
        Token::T_Script(sb, _) => *sb,
        _ => root,
    };
    if let Token::T_Literal(line) = ast.get(shebang) {
        let mut words = line.trim_start_matches("#!").split_whitespace();
        if let Some(interpreter) = words.next() {
            let mut rest: Vec<&str> = words.collect();
            if interpreter.rsplit('/').next() == Some("env") {
                // Skip env's own flags and assignments up to the shell name.
                let skip = rest.iter().position(|w| !w.starts_with('-') && !w.contains('=')).map_or(rest.len(), |i| i + 1);
                rest.drain(..skip);
            }
            seen.args(rest.into_iter());
        }
    }

    // `set` commands anywhere in the script
    let mut stack = vec![root];
    while let Some(t) = stack.pop() {
        if matches!(ast.get(t), Token::T_SimpleCommand(..)) && is_unqualified_command(ast, t, "set") {
            let words = oversimplify(ast, t);
            seen.args(words.iter().skip(1).map(String::as_str));
        }
        stack.extend(ast.children(t));
    }

    // Plain sh and dash have no pipefail to require.
    let needs_pipefail = matches!(params.shell_type, Shell::Bash | Shell::Ksh);
    let mut missing = vec![];
    if !seen.errexit {
        missing.push("errexit");
    }
    if !seen.nounset {
        missing.push("nounset");
    }
    if needs_pipefail && !seen.pipefail {
        missing.push("pipefail");
    }
    if missing.is_empty() {
        return vec![];
    }
    let suggestion = if needs_pipefail { "set -euo pipefail" } else { "set -eu" };
    let mut c = Comments::new();
    c.info(
        shebang,
        9001,
        format!("Script does not enable strict mode; missing {}. Consider '{suggestion}'.", missing.join(", ")),
    );
    c.0
}
