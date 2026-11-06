// SPDX-License-Identifier: GPL-3.0-only
// Tests of the pilot checks, the module checkers of ShellCheck (e11f964)
// src/ShellCheck/Analytics.hs `checker`, Checks/Commands.hs `checker` and
// Checks/ShellSupport.hs `checker` run on them, and crates/checks/src/support.rs.

use shellcheck_analyzer::analyzerlib::*;
use shellcheck_analyzer::api::*;
use shellcheck_analyzer::testing::{find_entries, run_check};
use shellcheck_checks::support::*;
use shellcheck_core::ast::{Ast, Id, Token};
use shellcheck_core::interface::*;

// ---- helpers ----

struct Parsed {
    ast: Ast,
    root: Id,
}

fn parse(s: &str) -> Parsed {
    let mut pr = p_script(s);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    Parsed { ast: pr.ast, root }
}

impl Parsed {
    fn params(&self, spec: &AnalysisSpec) -> Params<'_> {
        make_parameters(spec, &self.ast, self.root)
    }

    /// Pre-order list of the tokens matching `pred`.
    fn find(&self, pred: impl Fn(&Token) -> bool) -> Vec<Id> {
        let mut out = vec![];
        self.ast.analyze(
            self.root,
            &mut |x| {
                if pred(self.ast.get(x)) {
                    out.push(x)
                }
            },
            &mut |_| {},
        );
        out
    }

    fn literal(&self, s: &str) -> Id {
        self.find(|t| matches!(t, Token::T_Literal(l) if l == s))[0]
    }

    fn command(&self) -> Id {
        self.find(|t| matches!(t, Token::T_SimpleCommand(..)))[0]
    }

    fn word_with(&self, s: &str) -> Id {
        let lit = self.literal(s);
        self.ast.parent(lit).unwrap()
    }
}

fn spec() -> AnalysisSpec {
    AnalysisSpec { optional_checks: vec![], ..default_spec(&Default::default()) }
}

fn codes(v: &[TokenComment]) -> Vec<Code> {
    let mut c: Vec<Code> = v.iter().map(|c| c.comment.code).collect();
    c.sort();
    c
}

fn run_module(checker: &Checker, s: &str, spec: &AnalysisSpec) -> Vec<Code> {
    let p = parse(s);
    let params = p.params(spec);
    codes(&run_checker(&params, checker))
}

// ---- registration ----

#[test]
fn pilots_are_registered_by_module_and_name() {
    for (module, name, registry) in [
        ("Analytics.hs", "checkEchoWc", "node"),
        ("Analytics.hs", "checkShebangParameters", "tree"),
        ("Checks/Commands.hs", "checkTr", "command"),
        ("Checks/ShellSupport.hs", "checkForDecimals", "shell-support"),
        ("Analytics.hs", "checkForDecimals", "unregistered"),
    ] {
        let e = find_entries(module, name);
        assert_eq!(e.len(), 1, "{module}:{name}");
        assert_eq!(e[0].registry, registry);
    }
}

#[test]
fn pilot_codes() {
    let run = |m, n, k, s| run_check(m, n, k, s).unwrap();
    assert_eq!(run("Analytics.hs", "checkEchoWc", "node", "n=$(echo $foo | wc -m)"), [2000]);
    assert_eq!(run("Analytics.hs", "checkEchoWc", "node", "echo $foo | wc -l"), Vec::<Code>::new());
    assert_eq!(run("Analytics.hs", "checkShebangParameters", "tree", "#!/bin/bash -x -e\ntrue"), [2096]);
    assert_eq!(run("Checks/Commands.hs", "checkTr", "node", "tr 'a-z' 'A-Z'"), [2018, 2019]);
    assert_eq!(run("Checks/Commands.hs", "checkTr", "node", "tr [a] 'aa'"), [2020, 2060]);
    assert_eq!(run("Checks/Commands.hs", "checkTr", "node", "/usr/bin/tr '[abc]' x"), [2021]);
    assert_eq!(run("Checks/ShellSupport.hs", "checkForDecimals", "node", "((3.14*c))"), [2079]);
    // Same name, other module: the Analytics one honours hasFloatingPoint (ksh).
    assert_eq!(run("Analytics.hs", "checkForDecimals", "node", "#!/bin/ksh\n((3.14*c))"), Vec::<Code>::new());
    assert_eq!(run("Analytics.hs", "checkForDecimals", "node", "#!/bin/bash\n((3.14*c))"), [2079]);
}

// ---- module checkers (deferred from Task 13) ----

#[test]
fn analytics_checker_runs_node_and_tree_checks() {
    let c = analytics_checker(&spec());
    assert_eq!(run_module(&c, "#!/bin/bash -x -e\nn=$(echo $foo | wc -c)", &spec()), [2000, 2096]);
    // "unregistered" entries are never run by the module checker.
    assert_eq!(run_module(&c, "#!/bin/bash\n((3.14*c))", &spec()), Vec::<Code>::new());
}

#[test]
fn commands_checker_dispatches_by_basename() {
    let c = commands_checker(&spec());
    assert_eq!(run_module(&c, "tr a-z A-Z", &spec()), [2018, 2019]);
    assert_eq!(run_module(&c, "/bin/tr a-z x", &spec()), [2018]);
    // `builtin tr` looks up `Exactly "tr"` only; checkTr is `Basename "tr"`.
    assert_eq!(run_module(&c, "builtin tr a-z x", &spec()), Vec::<Code>::new());
    assert_eq!(run_module(&c, "echo a-z", &spec()), Vec::<Code>::new());
}

#[test]
fn shell_support_checker_honours_shell_list() {
    let s = "((3.14*c))";
    for (shebang, want) in [("#!/bin/sh", vec![2079]), ("#!/bin/bash", vec![2079]), ("#!/bin/ksh", vec![])] {
        let p = parse(&format!("{shebang}\n{s}"));
        let params = p.params(&spec());
        assert_eq!(codes(&run_checker(&params, &shell_support_checker(&params))), want, "{shebang}");
    }
}

// Test-only optional checks. They live in this test binary only.

fn opt_a(_: &Params<'_>, root: Id) -> Vec<TokenComment> {
    vec![make_comment(Severity::InfoC, root, 9001, "a")]
}
fn opt_b(_: &Params<'_>, root: Id) -> Vec<TokenComment> {
    vec![make_comment(Severity::InfoC, root, 9002, "b")]
}
fn opt_cmd(_: &Params<'_>, t: Id, c: &mut Comments) {
    c.info(t, 9003, "cmd");
}

const fn desc(name: &'static str) -> StaticCheckDescription {
    StaticCheckDescription { name, id: &[], description: "", positive: "", negative: "" }
}

#[linkme::distributed_slice(CHECKS)]
static OPT_A: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "optA",
    registry: "optional-tree",
    order: 0,
    optional: Some(desc("opt-a")),
    kind: CheckKind::Tree(opt_a),
};

#[linkme::distributed_slice(CHECKS)]
static OPT_B: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "optB",
    registry: "optional-tree",
    order: 1,
    optional: Some(desc("opt-b")),
    kind: CheckKind::Tree(opt_b),
};

#[linkme::distributed_slice(CHECKS)]
static OPT_CMD: CheckEntry = CheckEntry {
    module: "Test.hs",
    name: "optCmd",
    registry: "optional-command",
    order: 0,
    optional: Some(desc("opt-cmd")),
    kind: CheckKind::Command(CommandName::Exactly("tr"), opt_cmd),
};

#[test]
fn selected_optionals_all_versus_keys() {
    let with = |keys: &[&str]| AnalysisSpec { optional_checks: keys.iter().map(|k| k.to_string()).collect(), ..spec() };
    let script = "tr x y";
    let analytics = |keys: &[&str]| run_module(&analytics_checker(&with(keys)), script, &with(keys));
    let commands = |keys: &[&str]| run_module(&commands_checker(&with(keys)), script, &with(keys));
    assert_eq!(analytics(&[]), Vec::<Code>::new());
    assert_eq!(analytics(&["opt-b"]), [9002]);
    assert_eq!(analytics(&["opt-b", "nope", "opt-a"]), [9001, 9002]);
    assert_eq!(analytics(&["all"]), [9001, 9002]);
    assert_eq!(analytics(&["opt-cmd"]), Vec::<Code>::new());
    assert_eq!(commands(&[]), Vec::<Code>::new());
    assert_eq!(commands(&["opt-cmd"]), [9003]);
    assert_eq!(commands(&["all"]), [9003]);
    let names: Vec<String> = analytics_optional_checks().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["opt-a", "opt-b"]);
    assert_eq!(commands_optional_checks().len(), 1);
}

// ---- support.rs ----

#[test]
fn is_condition_walks_parents() {
    let p = parse("if a; b; then c; fi; d && e; f || g; while h; do i; done");
    let path = |s: &str| shellcheck_core::astlib::get_path(&p.ast, p.literal(s));
    // Only the last command of an if/while condition list counts.
    let want = [("a", false), ("b", true), ("c", false), ("d", true), ("e", false), ("f", true), ("g", false), ("h", true), ("i", false)];
    for (s, w) in want {
        assert_eq!(is_condition(&p.ast, &path(s)), w, "{s}");
    }
}

#[test]
fn functions_keeps_first_definition() {
    let p = parse("f() { :; }\ng() { :; }\nf() { true; }");
    let fs = functions(&p.ast, p.root);
    let defs = p.find(|t| matches!(t, Token::T_Function(..)));
    assert_eq!(fs.len(), 2);
    assert_eq!(fs["f"], defs[0]);
    assert_eq!(fs["g"], defs[1]);
}

#[test]
fn replacements_and_fixes() {
    let p = parse("echo $foo");
    let params = p.params(&spec());
    let word = p.find(|t| matches!(t, Token::T_DollarBraced(..)))[0];
    let depth = shellcheck_core::astlib::get_path(&p.ast, word).len() as i64;
    let at = |c| Position { file: String::new(), line: 1, column: c };
    let s = replace_start(word, &params, 1, "${");
    assert_eq!((s.start_pos, s.end_pos, s.string.as_str()), (at(6), at(7), "${"));
    assert_eq!((s.precedence, s.insertion_point), (depth, InsertionPoint::InsertAfter));
    let e = replace_end(word, &params, 0, "}");
    assert_eq!((e.start_pos, e.end_pos, e.insertion_point), (at(10), at(10), InsertionPoint::InsertBefore));
    let t = replace_token(word, &params, "x");
    assert_eq!((&t.start_pos, &t.end_pos), (&at(6), &at(10)));
    let f = surround_with(word, &params, "\"");
    assert_eq!(f.replacements.len(), 2);
    assert_eq!(f.replacements[0].start_pos, at(6));
    assert_eq!(f.replacements[1].end_pos, at(10));
    assert_eq!(fix_with(vec![t.clone()]).replacements, [t]);
}

#[test]
fn variable_flow_analysis_puts_later_output_first() {
    let p = parse("a=1; echo $a; b=2");
    let params = p.params(&spec());
    let out = do_variable_flow_analysis(
        |n: &mut u32, _, _, name| {
            *n += 1;
            vec![format!("read {name} {n}")]
        },
        |n: &mut u32, _, _, name, _| {
            *n += 1;
            vec![format!("write {name} {n}")]
        },
        0,
        &params.variable_flow,
    );
    assert_eq!(out, ["write b 3", "read a 2", "write a 1"]);
}

#[test]
fn is_sourced_and_floating_point() {
    let p = parse("#!/bin/ksh\necho a");
    let params = p.params(&spec());
    assert!(has_floating_point(&params));
    assert!(!is_sourced(&params, p.literal("a")));
}

#[test]
fn check_command_and_check_list() {
    let p = parse("/bin/sed -e x");
    let mut seen = None;
    check_command(&p.ast, "sed", |cmd, rest| seen = Some((cmd, rest.len())), p.command());
    assert_eq!(seen, Some((p.word_with("/bin/sed"), 2)));
    let mut none = true;
    check_command(&p.ast, "awk", |_, _| none = false, p.command());
    assert!(none);
    let double = |x: i32| vec![x, x];
    let single = |x: i32| vec![x + 1];
    assert_eq!(check_list(&[&double, &single], 3), [3, 3, 4]);
}

#[test]
fn run_node_analysis_visits_every_node() {
    let p = parse("a; b");
    let params = p.params(&spec());
    let f = |p: &Params<'_>, t: Id, c: &mut Comments| {
        if matches!(p.ast.get(t), Token::T_SimpleCommand(..)) {
            c.info(t, 1, "x");
        }
    };
    assert_eq!(codes(&run_node_analysis(&f, &params, p.root)), [1, 1]);
}

fn grep_codes(s: &str) -> Vec<Code> {
    let p = parse(s);
    let params = p.params(&spec());
    let grep = p.find(|t| matches!(t, Token::T_SimpleCommand(..))).into_iter().find(|&t| is_command(&p.ast, t, "grep")).unwrap();
    let mut c = Comments::new();
    check_grep_sends_pipefail_impl(&params, grep, &mut c);
    codes(&c.into_vec())
}

#[test]
fn grep_sends_pipefail() {
    assert_eq!(grep_codes("set -o pipefail; cat f | grep -q x"), [2337]);
    assert_eq!(grep_codes("set -o pipefail; cat f | grep --max-count 1 x"), [2337]);
    assert_eq!(grep_codes("cat f | grep -q x"), Vec::<Code>::new());
    assert_eq!(grep_codes("set -o pipefail; grep -q x f | cat"), Vec::<Code>::new());
    assert_eq!(grep_codes("set -o pipefail; cat f | grep -e -q"), Vec::<Code>::new());
}

fn exit_codes(s: &str) -> Vec<Code> {
    let p = parse(s);
    let mut c = Comments::new();
    return_or_exit(|id, c| c.err(id, 1, "multi"), |id, c| c.err(id, 2, "invalid"), &p.ast, p.command(), &mut c);
    codes(&c.into_vec())
}

#[test]
fn return_or_exit_classifies_arguments() {
    assert_eq!(exit_codes("exit 1 2"), [1]);
    assert_eq!(exit_codes("exit 255"), Vec::<Code>::new());
    assert_eq!(exit_codes("exit 256"), [2]);
    assert_eq!(exit_codes("exit -1"), [2]);
    assert_eq!(exit_codes("exit 000001"), [2]);
    assert_eq!(exit_codes("exit $x"), Vec::<Code>::new());
    assert_eq!(exit_codes("exit $((a))"), Vec::<Code>::new());
    assert_eq!(exit_codes("exit \"$x\"y"), [2]);
    assert_eq!(exit_codes("exit"), Vec::<Code>::new());
}

#[test]
fn single_unmodified_braced_string() {
    let get = |s: &str| {
        let p = parse(&format!("echo {s}"));
        let word = shellcheck_core::astlib::arguments(&p.ast, p.command())[0];
        get_single_unmodified_braced_string(&p.ast, word)
    };
    assert_eq!(get("$foo").as_deref(), Some("foo"));
    assert_eq!(get("\"$foo\"").as_deref(), Some("foo"));
    // The Haskell comment says "$foo"'' -> $foo, but `getWordParts` keeps
    // the empty T_SingleQuoted, so the code returns Nothing.
    assert_eq!(get("\"$foo\"''"), None);
    assert_eq!(get("${foo:-x}"), None);
    assert_eq!(get("\"hello $name\""), None);
}

fn mv_fires(s: &str) -> bool {
    let p = parse(s);
    let mut c = Comments::new();
    missing_destination(|id, c| c.err(id, 1, "x"), &p.ast, p.command(), &mut c);
    !c.into_vec().is_empty()
}

#[test]
fn missing_destination_cases() {
    assert!(mv_fires("mv 'foo bar'"));
    assert!(!mv_fires("mv foo bar"));
    assert!(!mv_fires("mv \"$@\""));
    assert!(!mv_fires("mv -t foo bar"));
    assert!(!mv_fires("mv --target-direc=foo bar"));
    assert!(!mv_fires("mv --version"));
}

#[test]
fn scratch_maps_synthetic_ids_back() {
    let p = parse("cp foo{1,2} bar");
    let word = p.word_with("foo");
    let mut s = Scratch::new(&p.ast);
    let words = s.brace_expand(word);
    assert_eq!(words.len(), 2);
    for &w in &words {
        assert!(w.0 >= p.ast.len(), "fresh node");
        assert_eq!(s.haskell_id(w), word);
        assert_eq!(s.parent(w), p.ast.parent(word));
    }
    assert_eq!(shellcheck_core::astlib::only_literal_string(&s.ast, words[1]), "foo2");
    // A synthetic token built from another keeps the original id.
    let cmd = p.command();
    let Token::T_SimpleCommand(pre, w) = p.ast.get(cmd).clone() else { panic!() };
    let t2 = s.synth(words[0], Token::T_SimpleCommand(pre, w[1..].to_vec()));
    assert_eq!(s.haskell_id(t2), word);
    assert_eq!(s.haskell_id(cmd), cmd);
    let params = p.params(&spec());
    let sp = s.params(&params);
    assert!(matches!(sp.ast.get(t2), Token::T_SimpleCommand(..)));
}
