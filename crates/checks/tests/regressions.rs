// SPDX-License-Identifier: GPL-3.0-or-later
// Regression tests for checks of the port of ShellCheck (e11f964) whose
// position, message or count diverged from the oracle; no Haskell
// counterpart (the Haskell tests only check whether a check fires).
//
// Expected values come from the oracle (shellcheck v0.11.0-171-ge11f964,
// `-f gcc --norc -o all`).

use shellcheck_analyzer::analyzerlib::*;
use shellcheck_analyzer::api::entry_test_checker;
use shellcheck_analyzer::testing::find_entries;
use shellcheck_core::interface::{AnalysisSpec, Shell, TokenComment};
use shellcheck_checks::CHECKS;

/// A comment as the oracle reports it: code, start (line, column), end
/// (line, column), message.
type Reported = (i64, (u32, u32), (u32, u32), String);

/// Run check `module`:`name` on `script`, the way its module's test helper
/// does, and return every comment it writes, positioned by its token's span
/// and in the order written.
fn report(module: &str, name: &str, script: &str) -> Vec<Reported> {
    let entries = find_entries(CHECKS, module, name);
    assert!(!entries.is_empty(), "{module}:{name} is registered");
    let checker = entry_test_checker(&entries);
    let mut pr = p_script(script);
    let root = pr.root.expect("script parses");
    pr.ast.compute_parents(root);
    let spec = AnalysisSpec { shell_type: Some(Shell::Bash), ..default_spec(&pr) };
    let params = make_parameters(&spec, &pr.ast, root);
    let comments: Vec<TokenComment> = run_checker(&params, &checker);
    comments
        .into_iter()
        .map(|tc| {
            let (s, e) = pr.ast.span(tc.id);
            (i64::from(tc.comment.code), (s.line, s.column), (e.line, e.column), tc.comment.message)
        })
        .collect()
}

fn starts(r: &[Reported]) -> Vec<(i64, (u32, u32))> {
    r.iter().map(|(code, s, _, _)| (*code, *s)).collect()
}

// ---- SC2249: on the case expression, not the case word ----

#[test]
fn default_case_covers_the_whole_case_statement() {
    let r = report("Analytics.hs", "checkDefaultCase", "case $1 in\n  a) true ;;\nesac\n");
    // oracle json1: line 1, column 1, endLine 3, endColumn 5
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].0, r[0].1, r[0].2), (2249, (1, 1), (3, 5)));
}

// ---- SC2316: on the offending argument, not the command ----

#[test]
fn multiple_declaring_points_at_the_argument() {
    let check = |s| starts(&report("Checks/Commands.hs", "checkMultipleDeclaring \"local\"", s));
    assert_eq!(check("Q() { local readonly var=1; }"), [(2316, (1, 13))]);
    let check = |s| starts(&report("Checks/Commands.hs", "checkMultipleDeclaring \"readonly\"", s));
    assert_eq!(check("readonly local foo=5"), [(2316, (1, 10))]);
    let check = |s| starts(&report("Checks/Commands.hs", "checkMultipleDeclaring \"export\"", s));
    assert_eq!(check("export readonly foo=5"), [(2316, (1, 8))]);
}

// ---- SC2261: FD numbers of any length ----

#[test]
fn pipe_to_nowhere_reads_fd_numbers_of_any_length() {
    let fd = "9".repeat(41);
    let script = format!("foo | cat {fd}>/dev/null 0{fd}>/dev/null\n");
    let r = report("Analytics.hs", "checkPipeToNowhere", &script);
    let msg = format!("Multiple redirections compete for FD {fd}. Use cat, tee, or pass filenames instead.");
    let got: Vec<(i64, (u32, u32), &str)> = r.iter().map(|(c, s, _, m)| (*c, *s, m.as_str())).collect();
    // `fromListWith (++)` prepends, so the later redirection comes first.
    assert_eq!(got, [(2261, (1, 105), msg.as_str()), (2261, (1, 52), msg.as_str())]);
}

#[test]
fn pipe_to_nowhere_still_names_standard_fds() {
    let r = report("Analytics.hs", "checkPipeToNowhere", "foo | cat 00>a 0<b\n");
    assert!(r.iter().any(|(c, _, _, m)| *c == 2261 && m.contains("compete for stdin")), "{r:?}");
}

// ---- SC2156: one comment per reached argument ----

#[test]
fn injectable_find_sh_comments_once_per_argument() {
    let mut script = String::from("find .");
    for _ in 0..20 {
        script += " -exec sh -c 'echo {}' \\;";
    }
    let r = report("Checks/Commands.hs", "checkInjectableFindSh \"find\"", &script);
    // Every 'echo {}' follows some -exec .. sh .. -c, each is warned once.
    assert_eq!(r.len(), 20);
    let mut ids: Vec<_> = r.iter().map(|x| x.1).collect();
    ids.dedup();
    assert_eq!(ids.len(), 20);
}

#[test]
fn injectable_find_sh_needs_ordered_exec_shell_dash_c() {
    let codes = |s: &str| starts(&report("Checks/Commands.hs", "checkInjectableFindSh \"find\"", s));
    assert_eq!(codes("find . -exec sh -c 'echo {}' \\;"), [(2156, (1, 20))]);
    assert_eq!(codes("find . -c 'x {}' sh -exec -c 'y {}'"), []);
    assert_eq!(codes("find . sh -exec bash -c 'y {}'"), [(2156, (1, 25))]);
    // -c without a following argument, and an argument after a second -c.
    assert_eq!(codes("find . -ok dash -c"), []);
    assert_eq!(codes("find . -ok dash x -c 'a' -c '{}'"), [(2156, (1, 29))]);
}

// ---- SC2114/SC2115 through brace expansion ----

#[test]
fn catastrophic_rm_expands_braces_without_copying_the_tree() {
    let r = |s: &str| starts(&report("Checks/Commands.hs", "checkCatastrophicRm \"rm\"", s));
    assert_eq!(r("rm -rf /{usr,var/lib}"), [(2114, (1, 8))]);
    assert_eq!(r("rm -r \"$d\"/{a,{b,usr}}"), [(2115, (1, 7))]);
    assert_eq!(r("rm -rf ${x:?}/usr \"$x\"/"), [(2115, (1, 19))]);
    assert_eq!(r("rm -rf /home/*"), [(2114, (1, 8))]);
    assert_eq!(r("rm -f /usr"), []);
}

// ---- SC2312 with a leading `time` ----

#[test]
fn extra_masked_returns_skips_a_leading_time() {
    let r = |s: &str| starts(&report("Analytics.hs", "checkExtraMaskedReturns", s));
    // removeTransparentCommands leaves `$(foo)` as the command name.
    assert_eq!(r("time $(foo)"), []);
    assert_eq!(r("time echo \"$(foo)\""), [(2312, (1, 14))]);
    // The dropped word is the first one, even when `time` is the
    // effective command.
    assert_eq!(r("command time $(foo) $(bar)"), [(2312, (1, 16)), (2312, (1, 23))]);
}

// ---- SC2154 suggestions ----

#[test]
fn unassigned_references_suggest_the_first_closest_name() {
    let r = report(
        "Analytics.hs",
        "checkUnassignedReferences",
        "variable_number=1; abcd1=1; abcd2=1; FOOBAR=1; longer_name=1\n\
         echo \"$variable_numbr\" \"$abcd3\" \"$foobar\" \"$lnger_nme\" \"$abc\"\n",
    );
    let msgs: Vec<&str> = r.iter().map(|x| x.3.as_str()).collect();
    assert_eq!(
        msgs,
        [
            "abc is referenced but not assigned.",
            "abcd3 is referenced but not assigned (did you mean 'abcd1'?).",
            "foobar is referenced but not assigned (did you mean 'FOOBAR'?).",
            "lnger_nme is referenced but not assigned (did you mean 'longer_name'?).",
            "variable_numbr is referenced but not assigned (did you mean 'variable_number'?).",
        ]
    );
}
