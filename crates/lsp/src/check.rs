// SPDX-License-Identifier: GPL-3.0-or-later
//! Running the checker on a buffer and turning its comments into LSP data.

use lsp_types::{DiagnosticSeverity, Position, Range, TextEdit};
use shelllint_checker::check_script;
use shelllint_core::interface::{CheckSpec, ParseSpec, PositionedComment, Severity, Shell, empty_check_spec};

use crate::config::Settings;
use crate::position::{split_lines, to_lsp};
use crate::sys::LspSystem;

pub const WIKI_LINK: &str = "https://www.shellcheck.net/wiki/";

/// One ShellCheck comment, already in LSP coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: i64,
    pub message: String,
    /// Edits of ShellCheck's suggested fix; empty when there is none.
    pub fix: Vec<TextEdit>,
}

impl Finding {
    pub fn contains(&self, pos: Position) -> bool {
        let r = self.range;
        pos >= r.start && (pos < r.end || (r.start == r.end && pos == r.start))
    }
}

pub fn lsp_severity(s: Severity) -> DiagnosticSeverity {
    match s {
        Severity::ErrorC => DiagnosticSeverity::ERROR,
        Severity::WarningC => DiagnosticSeverity::WARNING,
        Severity::InfoC => DiagnosticSeverity::INFORMATION,
        Severity::StyleC => DiagnosticSeverity::HINT,
    }
}

/// Run the full pipeline on `text`. `filename` picks the shell from its
/// extension and the rc files from its directory; with `ignore_rc` no rc or
/// editorconfig is read. A panic in the checker (a bug on some input) gives no
/// findings instead of killing the worker thread, which would silently stop
/// all further diagnostics.
pub fn run_check(text: &str, filename: &str, ignore_rc: bool, s: &Settings) -> Vec<Finding> {
    let spec = CheckSpec {
        filename: filename.to_string(),
        script: text.to_string(),
        ignore_rc,
        shell_type_override: s.dialect,
        excluded_warnings: s.exclude.clone(),
        min_severity: s.min_severity,
        optional_checks: s.enable.clone(),
        ..empty_check_spec()
    };
    let Ok(result) = std::panic::catch_unwind(|| check_script(&LspSystem, &spec)) else {
        return vec![];
    };
    let lines = split_lines(text);
    result.comments.iter().map(|pc| to_finding(pc, &lines)).collect()
}

fn to_finding(pc: &PositionedComment, lines: &[&str]) -> Finding {
    let mut range = Range {
        start: to_lsp(lines, pc.start_pos.line, pc.start_pos.column),
        end: to_lsp(lines, pc.end_pos.line, pc.end_pos.column),
    };
    if range.end <= range.start {
        // Zero-width comment: underline one character so it stays visible.
        range.end = Position { line: range.start.line, character: range.start.character + 1 };
    }
    let fix = pc
        .fix
        .as_ref()
        .map(|f| {
            f.replacements
                .iter()
                .map(|r| TextEdit {
                    range: Range {
                        start: to_lsp(lines, r.start_pos.line, r.start_pos.column),
                        end: to_lsp(lines, r.end_pos.line, r.end_pos.column),
                    },
                    new_text: r.string.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    Finding {
        range,
        severity: lsp_severity(pc.comment.severity),
        code: pc.comment.code,
        message: pc.comment.message.clone(),
        fix,
    }
}

/// A statement a directive can be placed in front of: its LSP span and the
/// line a directive would be inserted at.
struct Stmt {
    start: Position,
    end: Position,
    /// Line of the statement's first token.
    insert_line: u32,
}

/// Collects the statements of a parse tree that are list items (script,
/// brace group, subshell, if/loop/case/function bodies). It never descends
/// into words, so heredocs, quoted strings and command substitutions are
/// opaque: a diagnostic inside them belongs to the enclosing statement.
struct Collector<'a> {
    ast: &'a shelllint_core::ast::Ast,
    lines: &'a [&'a str],
    out: Vec<Stmt>,
}

impl Collector<'_> {
    fn lsp(&self, p: shelllint_core::ast::Pos) -> Position {
        to_lsp(self.lines, p.line as i64, p.column as i64)
    }

    fn starts_line(&self, pos: Position) -> bool {
        let Some(line) = self.lines.get(pos.line as usize) else { return false };
        let mut units = 0u32;
        for c in line.chars() {
            if units >= pos.character {
                break;
            }
            if c != ' ' && c != '\t' {
                return false;
            }
            units += c.len_utf16() as u32;
        }
        true
    }

    /// A list of statements: each that starts its own line is a candidate. A
    /// directive covers only the statement after it, so `cd d; rm $x` offers
    /// nothing for `rm`.
    fn list(&mut self, ids: &[shelllint_core::ast::Id]) {
        for &id in ids {
            let (s, e) = self.ast.span(id);
            let start = self.lsp(s);
            if self.starts_line(start) {
                let end = self.lsp(self.subtree_end(id, e));
                self.out.push(Stmt { start, end, insert_line: start.line });
            }
            self.structure(id);
        }
    }

    /// Furthest end of the token or any descendant: a heredoc body lies
    /// beyond the span of the command that owns it.
    fn subtree_end(&self, id: shelllint_core::ast::Id, mut end: shelllint_core::ast::Pos) -> shelllint_core::ast::Pos {
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            end = end.max(self.ast.span(i).1);
            stack.extend(self.ast.children(i));
        }
        end
    }

    /// Descend into the statement structure of `id` without treating `id`
    /// itself as a list item.
    fn structure(&mut self, id: shelllint_core::ast::Id) {
        use shelllint_core::ast::Token::*;
        match self.ast.get(id).clone() {
            T_Script(_, l) | T_BraceGroup(l) | T_Subshell(l) => self.list(&l),
            T_Annotation(_, c)
            | T_Backgrounded(c)
            | T_Banged(c)
            | T_Redirecting(_, c)
            | T_CoProc(_, c)
            | T_CoProcBody(c)
            | T_BatsTest(_, c) => self.structure(c),
            T_Function(_, _, _, c) => self.structure(c),
            T_Pipeline(_, cs) => cs.iter().for_each(|&c| self.structure(c)),
            T_AndIf(a, b) | T_OrIf(a, b) => {
                self.structure(a);
                self.structure(b);
            }
            T_IfExpression(branches, els) => {
                for (c, t) in &branches {
                    self.list(c);
                    self.list(t);
                }
                self.list(&els);
            }
            T_WhileExpression(c, b) | T_UntilExpression(c, b) => {
                self.list(&c);
                self.list(&b);
            }
            T_ForIn(_, _, b) | T_SelectIn(_, _, b) | T_ForArithmetic(_, _, _, b) => self.list(&b),
            T_CaseExpression(_, items) => items.iter().for_each(|(_, _, b)| self.list(b)),
            _ => {}
        }
    }
}

/// Codes a directive on line `l` disables, if it is a `# shellcheck` comment.
fn directive_disables(l: &str) -> Vec<String> {
    let Some(rest) = l.trim_start().strip_prefix('#') else { return vec![] };
    let mut words = rest.split_whitespace();
    if words.next() != Some("shellcheck") {
        return vec![];
    }
    let mut out = vec![];
    for w in words {
        if w.starts_with('#') {
            break;
        }
        if let Some(codes) = w.strip_prefix("disable=") {
            out.extend(codes.split(',').map(|c| c.trim().to_ascii_uppercase()));
        }
    }
    out
}

/// Edits that disable `SCxxxx` for the statement containing `pos`, by
/// inserting a `# shellcheck disable=` line above that statement with its
/// indentation. The statement is found in the parse tree, so the line is
/// never inside a heredoc, string, continuation, case item or `elif`. When no
/// safe insertion point exists, or the code is already disabled there, the
/// result is empty.
pub fn disable_edits(text: &str, filename: &str, dialect: Option<Shell>, pos: Position, code: i64) -> Vec<TextEdit> {
    let lines = split_lines(text);
    let spec = ParseSpec {
        filename: filename.to_string(),
        script: text.to_string(),
        check_sourced: false,
        ignore_rc: true,
        shell_type_override: dialect,
        ignore_directives: false,
    };
    let Ok(parsed) = std::panic::catch_unwind(|| shelllint_parser::parse_script(&LspSystem, &spec)) else {
        return vec![];
    };
    let Some(root) = parsed.root else { return vec![] };
    let mut c = Collector { ast: &parsed.ast, lines: &lines, out: vec![] };
    c.structure(root);
    // Innermost (latest-starting) statement containing the position.
    let Some(stmt) = c.out.iter().filter(|s| s.start <= pos && pos < s.end).max_by_key(|s| s.start) else {
        return vec![];
    };
    let at = stmt.insert_line as usize;
    let Some(target) = lines.get(at) else { return vec![] };
    // The line above must not continue onto this one.
    if at > 0 {
        let above = lines[at - 1].trim_end_matches(['\r']);
        if (above.len() - above.trim_end_matches('\\').len()) % 2 == 1 {
            return vec![];
        }
    }
    let tag = format!("SC{}", code);
    let mut i = at;
    while i > 0 && lines[i - 1].trim_start().starts_with('#') {
        i -= 1;
        if directive_disables(lines[i]).contains(&tag) {
            return vec![];
        }
    }
    let indent: String = target.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let p = Position { line: at as u32, character: 0 };
    vec![TextEdit { range: Range { start: p, end: p }, new_text: format!("{}# shellcheck disable={}\n", indent, tag) }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, edits: &[TextEdit]) -> String {
        assert_eq!(edits.len(), 1, "{edits:?}");
        let e = &edits[0];
        assert_eq!(e.range.start, e.range.end);
        assert_eq!(e.range.start.character, 0);
        let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
        lines.insert(e.range.start.line as usize, &e.new_text);
        lines.concat()
    }

    fn codes(text: &str) -> Vec<(i64, i64)> {
        let s = Settings::default();
        run_check(text, "t.sh", true, &s).iter().map(|f| (f.code, f.range.start.line as i64)).collect()
    }

    /// Disable `code` at its first finding; the edited script must no longer
    /// report it and must not gain any new finding (parse errors included).
    fn disable_ok(text: &str, code: i64, expect_line: u32, expect_text: &str) -> String {
        let s = Settings::default();
        let f = run_check(text, "t.sh", true, &s).into_iter().find(|f| f.code == code).expect("finding");
        let e = disable_edits(text, "t.sh", None, f.range.start, code);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].range.start.line, expect_line);
        assert_eq!(e[0].new_text, expect_text);
        let out = apply(text, &e);
        let before: Vec<i64> = codes(text).iter().map(|c| c.0).collect();
        let after: Vec<i64> = codes(&out).iter().map(|c| c.0).collect();
        assert!(!after.contains(&code) || before.iter().filter(|c| **c == code).count() > 1, "{out:?} {after:?}");
        for c in &after {
            assert!(before.contains(c), "new finding SC{c} in {out:?}");
        }
        out
    }

    #[test]
    fn inserts_directive_with_indentation() {
        disable_ok("if true; then\n    echo $x\nfi\n", 2086, 1, "    # shellcheck disable=SC2086\n");
    }

    #[test]
    fn keeps_shebang_on_top() {
        disable_ok("#!/bin/sh\necho $x\n", 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn trailing_comment_on_previous_directive_is_left_alone() {
        let t = "#!/bin/bash\n# shellcheck disable=SC2034 # reason here\necho $x\n";
        let out = disable_ok(t, 2086, 2, "# shellcheck disable=SC2086\n");
        assert!(out.contains("# reason here\n# shellcheck disable=SC2086\necho"));
    }

    #[test]
    fn source_directive_on_previous_line_is_left_alone() {
        let t = "#!/bin/bash\n# shellcheck disable=SC2034 source=/dev/null\necho $x\n";
        let out = disable_ok(t, 2086, 2, "# shellcheck disable=SC2086\n");
        assert!(out.contains("source=/dev/null\n"));
    }

    #[test]
    fn heredoc_goes_above_the_command() {
        disable_ok("#!/bin/bash\ncat <<EOF\n$(echo `date`)\nEOF\n", 2006, 1, "# shellcheck disable=SC2006\n");
    }

    #[test]
    fn multiline_string_goes_above_the_command() {
        disable_ok("#!/bin/bash\necho \"a\n$(echo `date`)\"\n", 2006, 1, "# shellcheck disable=SC2006\n");
    }

    #[test]
    fn continuation_goes_above_the_first_line() {
        disable_ok("#!/bin/bash\nfoo \\\n  $x\n", 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn case_item_goes_above_case() {
        disable_ok("#!/bin/bash\ncase $1 in\n  a) echo $x;;\nesac\n", 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn case_item_on_its_own_line_gets_indent() {
        disable_ok(
            "#!/bin/bash\ncase $1 in\n  a)\n    echo $x\n    ;;\nesac\n",
            2086,
            3,
            "    # shellcheck disable=SC2086\n",
        );
    }

    #[test]
    fn elif_goes_above_if() {
        let t = "#!/bin/bash\nif true; then :\nelif [ $x == 1 ]; then :; fi\n";
        disable_ok(t, 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn pipeline_continuation_goes_above_first_line() {
        disable_ok("#!/bin/bash\necho a |\n  cat $x\n", 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn second_statement_on_a_line_gets_no_action() {
        let t = "#!/bin/bash\ncd d; rm $x\n";
        assert!(disable_edits(t, "t.sh", None, Position { line: 1, character: 8 }, 2086).is_empty());
    }

    #[test]
    fn nested_statement_goes_above_the_enclosing_one() {
        disable_ok("#!/bin/bash\nf() {\n  cd d; rm $x\n}\n", 2086, 1, "# shellcheck disable=SC2086\n");
    }

    #[test]
    fn tab_indentation_is_kept() {
        disable_ok("#!/bin/bash\nif true; then\n\techo $x\nfi\n", 2086, 2, "\t# shellcheck disable=SC2086\n");
    }

    #[test]
    fn already_disabled_gives_no_action() {
        let t = "#!/bin/bash\n# shellcheck disable=SC2086,SC2034 # why\necho $x\n";
        assert!(disable_edits(t, "t.sh", None, Position { line: 2, character: 5 }, 2086).is_empty());
    }

    #[test]
    fn position_outside_any_statement_gives_no_action() {
        let t = "#!/bin/bash\n\n\necho hi\n";
        assert!(disable_edits(t, "t.sh", None, Position { line: 1, character: 0 }, 2086).is_empty());
        assert!(disable_edits("", "t.sh", None, Position { line: 0, character: 0 }, 2086).is_empty());
    }
}
