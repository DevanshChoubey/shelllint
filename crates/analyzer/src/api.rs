// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the check-registration machinery of ShellCheck (e11f964):
// src/ShellCheck/Analytics.hs (`checker`, `mkChecker`, `nodeChecksToTreeCheck`,
// `optionalChecks`), src/ShellCheck/Checks/Commands.hs (`CommandName`,
// `CommandCheck`, `buildCommandMap`, `checkCommand`, `getChecker`, `checker`,
// `optionalChecks`) and src/ShellCheck/Checks/ShellSupport.hs (`ForShell`,
// `getChecker`, `checker`, `testChecker`).
//
// DIVERGENCE: Haskell builds each module's check list by hand. Here every
// check registers a `CheckEntry` in the `CHECKS` distributed slice, tagged
// with its module, its registration list (`registry`) and its index in that
// list (`order`); `registered` rebuilds the list in Haskell order.
//
// Registries (as in corpus/tasks.json):
//   "node"             Analytics.hs `nodeChecks`
//   "tree"             Analytics.hs `treeChecks` (without the leading
//                      `nodeChecksToTreeCheck nodeChecks`)
//   "optional-tree"    Analytics.hs `optionalTreeChecks`
//   "command"          Checks/Commands.hs `commandChecks`
//   "optional-command" Checks/Commands.hs `optionalCommandChecks`
//   "shell-support"    Checks/ShellSupport.hs `checks`
// Any other registry name (e.g. "test") is reachable from
// `testing::run_check` but never run by the module checkers.

use std::collections::HashMap;

use shellcheck_core::ast::{Id, Token};
use shellcheck_core::interface::{AnalysisSpec, CheckDescription, Code, Shell, TokenComment};

use crate::analyzerlib::{Checker, Comments, Params};
use crate::astlib::{get_literal_string, only_literal_string};

/// A check run on every node: `Parameters -> Token -> Writer [TokenComment] ()`
/// (Analytics), `Token -> Analysis` (Commands, ShellSupport).
pub type NodeFn = fn(&Params<'_>, Id, &mut Comments);
/// A check run once on the root: `Parameters -> Token -> [TokenComment]`.
pub type TreeFn = fn(&Params<'_>, Id) -> Vec<TokenComment>;

/// Checks/Commands.hs `data CommandName = Exactly String | Basename String`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandName {
    Exactly(&'static str),
    Basename(&'static str),
}

pub enum CheckKind {
    /// An Analytics node check, run at every node in pre-order.
    Node(NodeFn),
    /// An Analytics tree check, run once with the root.
    Tree(TreeFn),
    /// `CommandCheck name f`: `f` gets the `T_SimpleCommand` of matching commands.
    Command(CommandName, NodeFn),
    /// `ForShell shells f`: `f` runs at every node when the shell is listed.
    ForShell(&'static [Shell], NodeFn),
}

/// `CheckDescription` with static fields, so it can sit in a `static`.
// DIVERGENCE: core's `CheckDescription` holds `String`s, which cannot be built
// in a `static` initializer; `description()` converts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticCheckDescription {
    pub name: &'static str,
    pub id: &'static [Code],
    pub description: &'static str,
    pub positive: &'static str,
    pub negative: &'static str,
}

impl StaticCheckDescription {
    pub fn description(&self) -> CheckDescription {
        CheckDescription {
            name: self.name.to_string(),
            id: self.id.to_vec(),
            description: self.description.to_string(),
            positive: self.positive.to_string(),
            negative: self.negative.to_string(),
        }
    }
}

pub struct CheckEntry {
    /// The Haskell module, as in corpus/tests.jsonl: "Analytics.hs",
    /// "Checks/Commands.hs", "Checks/ShellSupport.hs".
    pub module: &'static str,
    /// The Haskell name, e.g. "checkEchoWc". Parametrised command checks
    /// (`checkSudoArgs "sudo"` ...) register one entry per argument, all
    /// with the same name.
    pub name: &'static str,
    /// The Haskell list this check is registered in (see the file header).
    pub registry: &'static str,
    /// Index in that list.
    pub order: u32,
    /// For optional checks: their description (`cdName` is the key users
    /// pass to `--enable`).
    pub optional: Option<StaticCheckDescription>,
    pub kind: CheckKind,
}

#[linkme::distributed_slice]
pub static CHECKS: [CheckEntry];

/// The entries of one registration list, in Haskell order.
pub fn registered(registry: &str) -> Vec<&'static CheckEntry> {
    let mut out: Vec<&'static CheckEntry> = CHECKS.iter().filter(|e| e.registry == registry).collect();
    out.sort_by_key(|e| e.order);
    out
}

/// Turn one entry into a `Checker`, as the module's test helpers do:
/// Analytics `runNodeAnalysis f` / `f params root`, Commands
/// `getChecker [f]`, ShellSupport `testChecker` (which ignores the shell
/// list).
pub fn entry_test_checker(entries: &[&'static CheckEntry]) -> Checker {
    let mut checker = Checker::default();
    let mut commands = Vec::new();
    for e in entries {
        match &e.kind {
            CheckKind::Node(f) | CheckKind::ForShell(_, f) => checker.per_token.push(Box::new(*f)),
            CheckKind::Tree(f) => checker.per_script.push(tree_analysis(*f)),
            CheckKind::Command(name, f) => commands.push((*name, *f)),
        }
    }
    if !commands.is_empty() {
        checker = checker.compose(get_checker(&commands));
    }
    checker
}

fn tree_analysis(f: TreeFn) -> Box<crate::analyzerlib::Analysis> {
    Box::new(move |p: &Params<'_>, root: Id, c: &mut Comments| c.tell(f(p, root)))
}

/// `nodeChecksToTreeCheck checkList`: one pre-order walk calling every node
/// check in order at each node.
pub fn node_checks_to_tree_check(p: &Params<'_>, t: Id, check_list: &[NodeFn]) -> Vec<TokenComment> {
    let mut c = Comments::new();
    p.ast.analyze(
        t,
        &mut |x| {
            for f in check_list {
                f(p, x, &mut c);
            }
        },
        &mut |_| {},
    );
    c.into_vec()
}

/// Run an "node"/"tree"/"optional-tree" entry as an Analytics tree check.
fn analytics_tree_check(p: &Params<'_>, root: Id, e: &CheckEntry) -> Vec<TokenComment> {
    match &e.kind {
        CheckKind::Node(f) => node_checks_to_tree_check(p, root, &[*f]),
        CheckKind::Tree(f) => f(p, root),
        _ => panic!("{}: Analytics entries must be Node or Tree", e.name),
    }
}

/// Analytics.hs `optionalChecks`
pub fn analytics_optional_checks() -> Vec<CheckDescription> {
    optional_descriptions("optional-tree")
}

fn optional_descriptions(registry: &str) -> Vec<CheckDescription> {
    registered(registry)
        .into_iter()
        .filter_map(|e| e.optional.map(|d| d.description()))
        .collect()
}

/// `if "all" in keys then all optionals else mapMaybe (lookup key) keys`
fn selected_optionals(spec: &AnalysisSpec, registry: &str) -> Vec<&'static CheckEntry> {
    let all = registered(registry);
    let keys = &spec.optional_checks;
    if keys.iter().any(|k| k == "all") {
        all
    } else {
        keys.iter()
            .filter_map(|k| all.iter().copied().find(|e| e.optional.is_some_and(|d| d.name == k)))
            .collect()
    }
}

/// Analytics.hs `checker spec params` (`mkChecker spec params treeChecks`)
pub fn analytics_checker(spec: &AnalysisSpec) -> Checker {
    let node_checks: Vec<NodeFn> = registered("node")
        .into_iter()
        .map(|e| match e.kind {
            CheckKind::Node(f) => f,
            _ => panic!("{}: \"node\" entries must be CheckKind::Node", e.name),
        })
        .collect();
    let tree_checks = registered("tree");
    let optionals = selected_optionals(spec, "optional-tree");
    let per_script = move |p: &Params<'_>, root: Id, c: &mut Comments| {
        c.tell(node_checks_to_tree_check(p, root, &node_checks));
        for e in tree_checks.iter().chain(optionals.iter()) {
            c.tell(analytics_tree_check(p, root, e));
        }
    };
    Checker {
        per_script: vec![Box::new(per_script)],
        per_token: vec![],
    }
}

/// Checks/Commands.hs `optionalChecks`
pub fn commands_optional_checks() -> Vec<CheckDescription> {
    optional_descriptions("optional-command")
}

/// `buildCommandMap`
// DIVERGENCE: `M.Map CommandName (Token -> Analysis)` is split into one map
// per constructor, keyed by the name. `M.insertWith composeAnalyzers` runs
// the newly inserted function first, so each list is in reverse insertion
// order.
#[derive(Default)]
struct CommandMap {
    exactly: HashMap<&'static str, Vec<NodeFn>>,
    basename: HashMap<&'static str, Vec<NodeFn>>,
}

fn build_command_map(list: &[(CommandName, NodeFn)]) -> CommandMap {
    let mut map = CommandMap::default();
    for &(name, function) in list {
        let (m, k) = match name {
            CommandName::Exactly(k) => (&mut map.exactly, k),
            CommandName::Basename(k) => (&mut map.basename, k),
        };
        m.entry(k).or_default().insert(0, function);
    }
    map
}

fn run_all(fs: Option<&Vec<NodeFn>>, p: &Params<'_>, t: Id, c: &mut Comments) {
    for f in fs.into_iter().flatten() {
        f(p, t, c);
    }
}

/// `checkCommand`
fn check_command(map: &CommandMap, p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_SimpleCommand(cmd_prefix, words) = p.ast.get(t) else { return };
    let Some((&cmd, rest)) = words.split_first() else { return };
    let Some(name) = get_literal_string(p.ast, cmd) else { return };
    let basename = |s: &str| s.rsplit('/').next().unwrap_or("").to_string();
    if name.contains('/') {
        run_all(map.basename.get(basename(&name).as_str()), p, t, c);
    } else if name == "builtin" && !rest.is_empty() {
        let selected_builtin = only_literal_string(p.ast, rest[0]);
        let Some(fs) = map.exactly.get(selected_builtin.as_str()) else { return };
        // DIVERGENCE: `t' = T_SimpleCommand id cmdPrefix rest` is a synthetic
        // token with the same id. The checks run against a copy of the arena
        // with `t` replaced (only when one would run). The copy is made once
        // per script (`Params::builtin_scratch`); `t` is put back afterwards,
        // so every other token in it is the original. The copy keeps the
        // recorded parents, so `parent(rest[i])` is `t`, which in the copy is
        // the rewritten command; in Haskell `parentMap` maps rest[i] to the
        // original `builtin ...` token.
        let cell = &p.builtin_scratch.0;
        {
            let mut scratch = cell.borrow_mut();
            if scratch.as_ref().is_none_or(|a| a.len() != p.ast.len()) {
                *scratch = Some(p.ast.clone());
            }
            if let Some(a) = scratch.as_mut() {
                a.set(t, Token::T_SimpleCommand(cmd_prefix.clone(), rest.to_vec()));
            }
        }
        {
            let scratch = cell.borrow();
            let Some(ast) = scratch.as_ref() else { unreachable!() };
            let p2 = Params { ast, ..p.clone() };
            run_all(Some(fs), &p2, t, c);
        }
        if let Some(a) = cell.borrow_mut().as_mut() {
            a.set(t, p.ast.get(t).clone());
        }
    } else {
        run_all(map.exactly.get(name.as_str()), p, t, c);
        run_all(map.basename.get(name.as_str()), p, t, c);
    }
}

/// Checks/Commands.hs `getChecker`
pub fn get_checker(list: &[(CommandName, NodeFn)]) -> Checker {
    let map = build_command_map(list);
    Checker {
        per_script: vec![],
        per_token: vec![Box::new(move |p: &Params<'_>, t: Id, c: &mut Comments| {
            check_command(&map, p, t, c)
        })],
    }
}

fn command_check(e: &CheckEntry) -> (CommandName, NodeFn) {
    match e.kind {
        CheckKind::Command(name, f) => (name, f),
        _ => panic!("{}: command entries must be CheckKind::Command", e.name),
    }
}

/// Checks/Commands.hs `checker spec params`
pub fn commands_checker(spec: &AnalysisSpec) -> Checker {
    let list: Vec<(CommandName, NodeFn)> = registered("command")
        .into_iter()
        .chain(selected_optionals(spec, "optional-command"))
        .map(command_check)
        .collect();
    get_checker(&list)
}

/// Checks/ShellSupport.hs `checker params` (`getChecker params checks`)
pub fn shell_support_checker(params: &Params<'_>) -> Checker {
    let shell = params.shell_type;
    let per_token: Vec<Box<crate::analyzerlib::Analysis>> = registered("shell-support")
        .into_iter()
        .filter_map(|e| match e.kind {
            CheckKind::ForShell(list, a) => list.contains(&shell).then(|| {
                let b: Box<crate::analyzerlib::Analysis> = Box::new(a);
                b
            }),
            _ => panic!("{}: shell-support entries must be CheckKind::ForShell", e.name),
        })
        .collect();
    Checker {
        per_script: vec![],
        per_token,
    }
}
