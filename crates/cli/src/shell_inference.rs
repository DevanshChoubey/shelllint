// SPDX-License-Identifier: GPL-3.0-or-later
// Extensions beyond ShellCheck: `--fallback-shell`, `--infer-shell-from-name`
// and `--skip-unsupported-shebang`. Everything here is only reached when one
// of them is given, so default behaviour is untouched.
//
// DIVERGENCE: the three flags are matched by their exact long names (no
// getopt prefix abbreviations, not listed in `--help`), so that existing
// abbreviations such as `--in` keep meaning `--include`.

use shelllint_checker::shell_from_filename;
use shelllint_core::astlib::executable_from_shebang;
use shelllint_core::data::shell_for_executable;
use shelllint_core::interface::{Shell, SystemInterface};

use crate::options::Flag;

#[derive(Clone, Debug, Default)]
pub struct ShellInference {
    /// `--fallback-shell`: used only when the script names no shell itself.
    pub fallback: Option<Shell>,
    /// `--infer-shell-from-name`: guess the shell from `.bashrc`, `.kshrc`, ...
    pub infer_from_name: bool,
    /// `--skip-unsupported-shebang`: skip (as clean) files with no usable shell.
    pub skip_unsupported: bool,
}

impl ShellInference {
    fn active(&self) -> bool {
        self.fallback.is_some() || self.infer_from_name || self.skip_unsupported
    }
}

/// Pulls the new flags out of `argv` (up to a `--`), returning them as
/// `Flag`s plus the remaining arguments for getopt.
pub fn extract(argv: &[String]) -> (Vec<Flag>, Vec<String>) {
    let mut flags = vec![];
    let mut rest = vec![];
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        i += 1;
        if a == "--" {
            rest.push(a.clone());
            rest.extend(argv[i..].iter().cloned());
            break;
        }
        match a.as_str() {
            "--infer-shell-from-name" => flags.push(Flag("infer-shell-from-name".into(), "true".into())),
            "--skip-unsupported-shebang" => flags.push(Flag("skip-unsupported-shebang".into(), "true".into())),
            "--fallback-shell" => {
                let v = argv.get(i).cloned().unwrap_or_default();
                i = (i + 1).min(argv.len());
                flags.push(Flag("fallback-shell".into(), v));
            }
            _ => match a.strip_prefix("--fallback-shell=") {
                Some(v) => flags.push(Flag("fallback-shell".into(), v.to_string())),
                None => rest.push(a.clone()),
            },
        }
    }
    (flags, rest)
}

pub enum Decision {
    /// Check as usual, with this shell forced.
    Use(Shell),
    /// Do not check the file at all; it counts as clean.
    Skip,
}

/// The shell a file name implies (`--infer-shell-from-name`).
pub fn shell_from_name(filename: &str) -> Option<Shell> {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    match base {
        ".bashrc" | ".bash_profile" | ".bash_login" | ".bash_logout" | ".bash_aliases" => return Some(Shell::Bash),
        ".kshrc" => return Some(Shell::Ksh),
        ".profile" | ".shrc" => return Some(Shell::Sh),
        _ => {}
    }
    if base.ends_with(".bash") {
        Some(Shell::Bash)
    } else if base.ends_with(".ksh") {
        Some(Shell::Ksh)
    } else {
        None
    }
}

/// `shell=NAME` in a `# shellcheck` directive of the leading comment block.
fn has_shell_directive(script: &str) -> bool {
    for line in script.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let Some(body) = t.strip_prefix('#') else { break };
        let body = body.trim_start();
        if let Some(args) = body.strip_prefix("shellcheck") {
            if args.starts_with(char::is_whitespace) && args.split_whitespace().any(|w| w.starts_with("shell=")) {
                return true;
            }
        }
    }
    false
}

fn rc_gives_shell(contents: &str) -> bool {
    contents.lines().any(|l| {
        let l = l.trim_start();
        !l.starts_with('#') && l.strip_prefix("shell").is_some_and(|r| r.trim_start().starts_with('='))
    })
}

#[derive(PartialEq)]
enum Shebang {
    None,
    Shell,
    NotShell,
}

fn classify_shebang(script: &str) -> Shebang {
    let script = script.strip_prefix('\u{FEFF}').unwrap_or(script);
    let Some(rest) = script.strip_prefix("#!") else { return Shebang::None };
    let line = rest.lines().next().unwrap_or("");
    let exe = executable_from_shebang(line);
    // Mirrors the parser's `isValidShell`: empty and known prefixes are fine.
    let good = ["sh", "ash", "dash", "busybox sh", "bash", "bats", "ksh", "oksh"];
    if exe.is_empty() && line.trim().is_empty() {
        Shebang::None
    } else if exe.is_empty() || good.iter().any(|g| exe.starts_with(g)) || shell_for_executable(&exe).is_some() {
        Shebang::Shell
    } else {
        Shebang::NotShell
    }
}

/// Decides what to do with one file. `None` means: check exactly as before.
pub fn decide(
    cfg: &ShellInference,
    shell_flag: bool,
    ignore_rc: bool,
    sys: &dyn SystemInterface,
    filename: &str,
    script: &str,
) -> Option<Decision> {
    if !cfg.active() || shell_flag {
        return None;
    }
    if has_shell_directive(script) {
        return None;
    }
    match classify_shebang(script) {
        Shebang::Shell => return None,
        Shebang::NotShell => return cfg.skip_unsupported.then_some(Decision::Skip),
        Shebang::None => {}
    }
    if !ignore_rc && sys.get_config(filename).is_some_and(|(_, c)| rc_gives_shell(&c)) {
        return None;
    }
    let inferred = if cfg.infer_from_name { shell_from_name(filename) } else { None };
    if let Some(sh) = inferred.or(cfg.fallback) {
        return Some(Decision::Use(sh));
    }
    if cfg.skip_unsupported && shell_from_filename(filename).is_none() {
        return Some(Decision::Skip);
    }
    None
}
