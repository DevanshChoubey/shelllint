// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Checker.hs and src/ShellCheck/Analyzer.hs (e11f964),
// plus `decodeString` from shellcheck.hs (the CLI's file decoding).
//
// Not ported: the TemplateHaskell `runTests` footer and the test helpers
// (`getErrors`, `check`, ...); the `prop_` definitions live in
// `crates/checker/tests/checker.rs`.
#![forbid(unsafe_code)]

pub mod editorconfig;

use std::collections::HashMap;

// The checks register into the analyzer's `CHECKS` slice via linkme; nothing
// references the checks crate by name, so without this it is not linked and
// no check ever runs.
use shellcheck_checks as _;

use shellcheck_analyzer::analyzerlib::{Checker, Params, filter_by_annotation, make_parameters, run_checker};
use shellcheck_analyzer::api::{
    analytics_checker, analytics_optional_checks, commands_checker, commands_optional_checks,
    shell_support_checker,
};
use shellcheck_core::ast::{Ast, Id};
use shellcheck_core::astlib::get_enable_directives;
use shellcheck_core::interface::{
    AnalysisResult, AnalysisSpec, CheckDescription, CheckResult, CheckSpec, Code, ExecutionMode, ParseSpec,
    PositionedComment, Severity, Shell, SystemInterface, TokenComment, empty_check_result, empty_check_spec,
    mocked_system_interface, new_analysis_result, new_analysis_spec, new_parse_spec, new_positioned_comment,
};
use shellcheck_parser::{ParseResult, parse_script};

// ---- Analyzer.hs ---------------------------------------------------------

// TODO: Clean up the cruft this is layered on
/// `analyzeScript`. Records `ast`'s parents for `root` first.
// DIVERGENCE: `asScript` is passed as the arena and its root id (core's
// `AnalysisSpec` has no AST).
pub fn analyze_script(spec: &AnalysisSpec, ast: &mut Ast, root: Id) -> AnalysisResult {
    ast.compute_parents(root);
    let ast: &Ast = ast;
    let params = make_parameters(spec, ast, root);
    let notes = nub_token_comments(run_checker(&params, &checkers(spec, &params)));
    AnalysisResult { comments: filter_by_annotation(spec, &params, notes), ..new_analysis_result() }
}

/// `checkers spec params`
// Checks/ControlFlow.hs (Phase 3) is not ported and Checks/Custom.hs is an
// empty checker (`nullCheck`); neither contributes anything yet.
fn checkers(spec: &AnalysisSpec, params: &Params<'_>) -> Checker {
    Checker::mconcat([analytics_checker(spec), commands_checker(spec), shell_support_checker(params)])
}

/// `optionalChecks`
// Checks/ControlFlow.hs `optionalChecks` is not ported yet (Phase 3).
pub fn optional_checks() -> Vec<CheckDescription> {
    let mut v = analytics_optional_checks();
    v.extend(commands_optional_checks());
    v
}

/// `nub` on token comments.
// DIVERGENCE: Haskell's `nub` is quadratic. Equal comments have the same id
// and code, so each comment is only compared with the kept comments sharing
// those; the result (first occurrences, in order) is the same.
fn nub_token_comments(list: Vec<TokenComment>) -> Vec<TokenComment> {
    let mut seen: HashMap<(Id, Code), Vec<usize>> = HashMap::new();
    let mut out: Vec<TokenComment> = Vec::new();
    for c in list {
        let kept = seen.entry((c.id, c.comment.code)).or_default();
        if kept.iter().any(|&i| out[i] == c) {
            continue;
        }
        kept.push(out.len());
        out.push(c);
    }
    out
}

// ---- Checker.hs ----------------------------------------------------------

/// `tokenToPosition startMap t`
fn token_to_position(pr: &ParseResult, t: TokenComment) -> PositionedComment {
    let Some((start, end)) = pr.token_positions(t.id) else {
        panic!("Internal shellcheck error: id doesn't exist. Please report!");
    };
    PositionedComment { start_pos: start, end_pos: end, comment: t.comment, fix: t.fix, ..new_positioned_comment() }
}

pub fn shell_from_filename(filename: &str) -> Option<Shell> {
    let shell_extensions = [
        (".ksh", Shell::Ksh),
        (".bash", Shell::Bash),
        (".bats", Shell::Bash),
        (".dash", Shell::Dash),
        (".envrc", Shell::Bash),
    ];
    // The `.sh` is too generic to determine the shell:
    // We fallback to Bash in this case and emit SC2148 if there is no shebang
    shell_extensions.iter().find(|(ext, _)| filename.ends_with(ext)).map(|&(_, sh)| sh)
}

/// Stack size of the thread `check_script` parses and analyses on.
// Measured on 5000 nested `$(`: one nesting level costs roughly 45 KB of
// stack in a debug build and 15-25 KB in release, so 64 MB overflows at
// about 1500 (debug) / 3000-5000 (release) levels. 512 MB is address space
// reserved for the thread, not memory: only the pages actually used are
// committed.
pub const CHECK_STACK_BYTES: usize = 512 * 1024 * 1024;

/// `checkScript`
// DIVERGENCE: the parser's readers recurse once per nesting level, like the
// Haskell (whose stack lives on the heap and grows to most of RAM). Parsing
// and analysis run on a fresh thread with a `CHECK_STACK_BYTES` stack, so
// deeply nested input (5000 nested `$(` needs ~225 MB in a debug build)
// cannot overflow; nesting beyond ~10000 levels still aborts the process.
// This is why `SystemInterface` requires `Sync`.
pub fn check_script(sys: &dyn SystemInterface, spec: &CheckSpec) -> CheckResult {
    shellcheck_parser::engine::with_stack(CHECK_STACK_BYTES, || {
        let results = check_script_contents(sys, spec, &spec.script);
        CheckResult { filename: spec.filename.clone(), comments: results, ..empty_check_result() }
    })
}

/// The `where`-bound `checkScript contents` of `checkScript`.
fn check_script_contents(sys: &dyn SystemInterface, spec: &CheckSpec, contents: &str) -> Vec<PositionedComment> {
    let mut result = parse_script(
        sys,
        &ParseSpec {
            filename: spec.filename.clone(),
            script: contents.to_string(),
            check_sourced: spec.check_sourced,
            ignore_rc: spec.ignore_rc,
            shell_type_override: spec.shell_type_override,
            ..new_parse_spec()
        },
    );
    let parse_messages = std::mem::take(&mut result.comments);
    let analysis_spec = |ast: &Ast, root: Id| {
        let mut optional_checks = get_enable_directives(ast, root);
        optional_checks.extend(spec.optional_checks.iter().cloned());
        AnalysisSpec {
            shell_type: spec.shell_type_override,
            fallback_shell: shell_from_filename(&spec.filename),
            check_sourced: spec.check_sourced,
            execution_mode: ExecutionMode::Executed,
            extended_analysis: spec.extended_analysis,
            optional_checks,
            ..new_analysis_spec()
        }
    };
    let analysis_messages = match result.root {
        None => vec![],
        Some(root) => {
            let a_spec = analysis_spec(&result.ast, root);
            analyze_script(&a_spec, &mut result.ast, root).comments
        }
    };
    let mut all = parse_messages;
    all.extend(analysis_messages.into_iter().map(|t| token_to_position(&result, t)));
    let included: Vec<PositionedComment> = all.into_iter().filter(|pc| should_include(spec, pc)).collect();
    nub_sorted(sort_messages(included))
}

fn should_include(spec: &CheckSpec, pc: &PositionedComment) -> bool {
    let code = pc.comment.code;
    let severity = pc.comment.severity;
    severity <= spec.min_severity
        && match &spec.included_warnings {
            None => !spec.excluded_warnings.contains(&code),
            Some(included_warnings) => included_warnings.contains(&code),
        }
}

/// `sortMessages = sortOn order`: by file, line, column, severity, code,
/// message (stable).
pub fn sort_messages(mut list: Vec<PositionedComment>) -> Vec<PositionedComment> {
    list.sort_by(|a, b| order(a).cmp(&order(b)));
    list
}

fn order(pc: &PositionedComment) -> (&str, i64, i64, Severity, Code, &str) {
    let pos = &pc.start_pos;
    let comment = &pc.comment;
    (&pos.file, pos.line, pos.column, comment.severity, comment.code, &comment.message)
}

/// `nub` on a list sorted by `order`.
// DIVERGENCE: Haskell's `nub` is quadratic. Equal comments have equal sort
// keys, so on a sorted list each comment is only compared with the kept
// comments in its run of equal keys; the result is the same.
fn nub_sorted(list: Vec<PositionedComment>) -> Vec<PositionedComment> {
    let mut out: Vec<PositionedComment> = Vec::with_capacity(list.len());
    let mut run_start = 0;
    for c in list {
        if out.last().is_none_or(|l| order(l) != order(&c)) {
            run_start = out.len();
        }
        if !out[run_start..].contains(&c) {
            out.push(c);
        }
    }
    out
}

// ---- harness entry point ---------------------------------------------------

/// The codes the full pipeline reports for `script`, as the oracle was run
/// for the corpus (`shellcheck -f json1 --norc -o all [-s shell] tNNNNN.sh`):
/// `mockedSystemInterface []` (nothing can be sourced), no rc file, every
/// optional check enabled. In report order.
pub fn check_script_codes(script: &str, shell: Option<Shell>) -> Vec<Code> {
    let spec = CheckSpec {
        filename: "script.sh".to_string(),
        script: script.to_string(),
        ignore_rc: true,
        shell_type_override: shell,
        optional_checks: vec!["all".to_string()],
        ..empty_check_spec()
    };
    check_script(&mocked_system_interface(vec![]), &spec).comments.into_iter().map(|pc| pc.comment.code).collect()
}

// ---- shellcheck.hs -----------------------------------------------------------

/// shellcheck.hs `decodeString`: decode a byte string as UTF-8, falling
/// back to ISO-8859-1 for each byte that does not start a valid sequence.
/// Like the Haskell, overlong forms are accepted.
// DIVERGENCE: the Haskell decodes UTF-16 surrogate code points (U+D800 to
// U+DFFF) to a `Char`; Rust's `char` cannot hold them, so they become
// U+FFFD, still one character (columns match the oracle's).
pub fn decode_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii() {
            out.push(c as char);
            i += 1;
            continue;
        }
        let num = c as u32;
        let rest = &bytes[i + 1..];
        let next = if num >= 0xF8 {
            None
        } else if num >= 0xF0 {
            construct(num & 0x07, 3, rest)
        } else if num >= 0xE0 {
            construct(num & 0x0F, 2, rest)
        } else if num >= 0xC0 {
            construct(num & 0x1F, 1, rest)
        } else {
            None
        };
        match next {
            Some((n, used)) => {
                out.push(char::from_u32(n).unwrap_or('\u{FFFD}'));
                i += 1 + used;
            }
            None => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

/// `construct x n rest`: the code point and the number of continuation
/// bytes consumed.
fn construct(mut x: u32, n: usize, rest: &[u8]) -> Option<(u32, usize)> {
    for k in 0..n {
        let num = *rest.get(k)? as u32;
        if !(0x80..=0xBF).contains(&num) {
            return None;
        }
        x = (x << 6) | (num & 0x3f);
    }
    (x <= 0x10FFFF).then_some((x, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellcheck_core::interface::{Comment, Position};

    fn pc(line: i64, code: Code, end: i64) -> PositionedComment {
        let pos = |column| Position { file: "f".into(), line, column };
        PositionedComment {
            start_pos: pos(1),
            end_pos: pos(end),
            comment: Comment { severity: Severity::ErrorC, code, message: "m".into() },
            ..new_positioned_comment()
        }
    }

    #[test]
    fn nub_sorted_keeps_first_occurrences() {
        // Same key, different end positions: both kept, duplicates dropped.
        let list = sort_messages(vec![pc(1, 1, 5), pc(1, 1, 3), pc(1, 1, 5), pc(2, 1, 5), pc(1, 1, 3)]);
        let got: Vec<(i64, i64)> = nub_sorted(list).iter().map(|p| (p.start_pos.line, p.end_pos.column)).collect();
        assert_eq!(got, vec![(1, 5), (1, 3), (2, 5)]);
    }

    #[test]
    fn nub_token_comments_keeps_first_occurrences() {
        let tc = |id: u32, code: Code, msg: &str| TokenComment {
            id: Id(id),
            comment: Comment { severity: Severity::InfoC, code, message: msg.into() },
            fix: None,
        };
        let got = nub_token_comments(vec![tc(1, 2, "a"), tc(1, 2, "b"), tc(1, 2, "a"), tc(2, 2, "a"), tc(2, 2, "a")]);
        assert_eq!(got, vec![tc(1, 2, "a"), tc(1, 2, "b"), tc(2, 2, "a")]);
    }

    #[test]
    fn shell_from_filename_cases() {
        assert_eq!(shell_from_filename("a.ksh"), Some(Shell::Ksh));
        assert_eq!(shell_from_filename("x/.envrc"), Some(Shell::Bash));
        assert_eq!(shell_from_filename("a.bats"), Some(Shell::Bash));
        assert_eq!(shell_from_filename("a.dash"), Some(Shell::Dash));
        assert_eq!(shell_from_filename("a.sh"), None);
    }
}
