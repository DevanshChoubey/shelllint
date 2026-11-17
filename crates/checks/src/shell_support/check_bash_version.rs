// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own check family SC9100-SC9199, `--bash-version`. There is
// no Haskell counterpart. It warns about bash features newer than the release the
// user says the script must run on. Off unless a target is set, and only for bash.
//
// Layout (the build script only registers `.rs` files directly in this directory,
// hence this file plus a directory instead of `check_bash_version/mod.rs`):
// - this file: the feature table, the registered entry and the two emit helpers
// - `check_bash_version/syntax.rs`: SC9104 9105 9106 9109 9113 9137
// - `check_bash_version/commands.rs`: builtins and their flags
// - `check_bash_version/expansions.rs`: `${..}` and brace expansions
// - `check_bash_version/variables.rs`: variables, `+=`, `[[ -v ]]` and array indexes
//
// Every sub-check is `pub fn check(p: &Params<'_>, t: Id, c: &mut Comments)`, run once
// per node. A sub-check reports through `too_new` / `too_new_as` only.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::Id;
use shelllint_core::interface::{BashVersion, Code, Shell};

#[path = "check_bash_version/commands.rs"]
mod commands;
#[path = "check_bash_version/expansions.rs"]
mod expansions;
#[path = "check_bash_version/syntax.rs"]
mod syntax;
#[path = "check_bash_version/variables.rs"]
mod variables;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Checks/ShellSupport.hs",
    name: "checkBashVersion",
    registry: "shell-support",
    order: 9,
    optional: None,
    kind: CheckKind::ForShell(&[Shell::Bash], f),
};

/// One bash feature: the code, the release that introduced it, and its name in the message.
pub struct Feature {
    pub code: Code,
    pub version: BashVersion,
    pub name: &'static str,
}

const fn row(code: Code, major: u8, minor: u8, name: &'static str) -> Feature {
    Feature { code, version: BashVersion::release(major, minor), name }
}

/// Every feature, in code order. Versions are checked against bash's NEWS.
pub const FEATURES: [Feature; 38] = [
    row(9100, 3, 1, "printf -v"),
    row(9101, 3, 1, "+= append"),
    row(9102, 4, 0, "declare -A"),
    row(9103, 4, 0, "mapfile"),
    row(9104, 4, 0, "|&"),
    row(9105, 4, 0, ";& and ;;& case terminators"),
    row(9106, 4, 0, "coproc"),
    row(9107, 4, 0, "case modification in ${..}"),
    row(9108, 4, 0, "zero-padded or stepped brace expansion"),
    row(9109, 4, 0, "&>>"),
    row(9110, 4, 0, "$BASHPID"),
    row(9111, 4, 0, "shopt -s globstar"),
    row(9112, 4, 0, "fractional read -t"),
    row(9113, 4, 1, "{fd}> fd allocation"),
    row(9114, 4, 1, "read -N"),
    row(9115, 4, 1, "printf -v with an array element"),
    row(9116, 4, 2, "declare -g"),
    row(9117, 4, 2, "negative index or length in ${..}"),
    row(9118, 4, 2, "-v variable test"),
    row(9119, 4, 2, "shopt -s lastpipe"),
    row(9120, 4, 2, "printf %(fmt)T"),
    row(9121, 4, 3, "negative array index assignment"),
    row(9122, 4, 3, "declare -n"),
    row(9123, 4, 3, "[[ -R ]]"),
    row(9124, 4, 3, "wait -n"),
    row(9125, 4, 4, "${v@Q} style transformation"),
    row(9126, 4, 4, "local -"),
    row(9127, 4, 4, "mapfile -d"),
    row(9128, 4, 4, "shopt -s inherit_errexit"),
    row(9129, 5, 0, "$EPOCHSECONDS, $EPOCHREALTIME or $BASH_ARGV0"),
    row(9130, 5, 0, "wait -f"),
    row(9131, 5, 0, "shopt -s localvar_inherit or localvar_unset"),
    row(9132, 5, 1, "$SRANDOM"),
    row(9133, 5, 1, "wait -p"),
    row(9134, 5, 1, "${v@U} style transformation"),
    row(9135, 5, 2, "${v@k}"),
    row(9136, 5, 2, "shopt -s patsub_replacement"),
    row(9137, 5, 3, "${ cmd; } command substitution"),
];

/// The table row for `code`.
pub fn feature(code: Code) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.code == code)
}

/// The target release, when the check is on: a target is set and the shell is bash.
pub fn target(p: &Params<'_>) -> Option<BashVersion> {
    if p.shell_type == Shell::Bash { p.bash_version } else { None }
}

/// Warn that the feature of `code` is newer than the target, at node `t`.
/// Says nothing when there is no target, the shell is not bash, or the target is
/// the feature's release or later. The message is
/// `<feature> requires bash <V> (target is <T>).`
pub fn too_new(p: &Params<'_>, c: &mut Comments, t: Id, code: Code) {
    too_new_as(p, c, t, code, None);
}

/// Like `too_new`, but `what` replaces the table's feature name in the message,
/// for rows that cover several spellings (`;&` versus `;;&`, `$SRANDOM`).
pub fn too_new_as(p: &Params<'_>, c: &mut Comments, t: Id, code: Code, what: Option<&str>) {
    let Some(target) = target(p) else { return };
    let Some(f) = feature(code) else {
        debug_assert!(false, "SC{code} is not in the bash version table");
        return;
    };
    if target < f.version {
        c.warn(t, code, format!("{} requires bash {} (target is {}).", what.unwrap_or(f.name), f.version, target));
    }
}

fn f(p: &Params<'_>, t: Id, c: &mut Comments) {
    if target(p).is_none() {
        return;
    }
    syntax::check(p, t, c);
    commands::check(p, t, c);
    expansions::check(p, t, c);
    variables::check(p, t, c);
}
