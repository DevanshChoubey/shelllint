// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: extension beyond ShellCheck (`--report-unused-disables`, SC9006).
//
// Reports each `disable=` code of an in-file directive that suppressed
// nothing, on the directive's line.
//
// How "suppressed nothing" is decided. The checker runs the whole pipeline
// twice on the script: once honouring in-file `disable=` directives and once
// ignoring them. A message that only appears in the second run was suppressed
// by a directive. An element of a `disable=` list (`SC2086`, `SC2000-SC2010`,
// `all`) is used when some such message has a code in the element's range and
// lies inside the code the directive applies to (the child of its
// `T_Annotation`: the whole file for a directive before the first command,
// otherwise the command that follows it). This covers parse-time messages
// (SC1xxx) as well as analysis messages. Every directive that covers a
// suppressed message counts as used, so nested directives for the same code
// are not reported as redundant.
//
// The comparison is made before the `-S`, `-i` and `-e` filters, so those
// flags never make a directive look unused.
//
// Optional checks: an optional check that is not enabled never produces a
// message, so a directive for it cannot be judged. An element is therefore
// never reported when its range contains the code of an optional check that
// is not enabled (by `--enable`, `enable=` directives or `all`). This also
// means `disable=all` is only reported when every optional check is enabled.
//
// Not judged: directives whose lines the parser attached to no command (for
// example inside a here-document), and checks that `--extended-analysis=false`
// skips (a directive for one of them looks unused).

use std::collections::HashSet;

use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::get_enable_directives;
use shelllint_core::interface::{CheckSpec, Code, Position, PositionedComment, Severity, new_positioned_comment};
use shelllint_parser::ParseResult;

/// The code of the "unused disable" report.
pub const UNUSED_DISABLE_CODE: Code = 9006;

/// One element of a `disable=` list.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Element {
    /// [from, to)
    from: i64,
    to: i64,
    label: String,
}

/// A directive comment line with at least one `disable=` element.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Directive {
    line: i64,
    column: i64,
    end_column: i64,
    elements: Vec<Element>,
}

fn is_line_space(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Parse `SC1234`, `1234`, `SC1000-SC1010` or `all`.
fn parse_element(s: &str) -> Option<Element> {
    let s = s.trim();
    if s == "all" {
        return Some(Element { from: 0, to: 1_000_000, label: "all".to_string() });
    }
    fn code(s: &str) -> Option<i64> {
        let s = s.strip_prefix("SC").unwrap_or(s);
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(s.parse::<i64>().unwrap_or(i64::MAX))
    }
    match s.split_once('-') {
        None => {
            let n = code(s)?;
            Some(Element { from: n, to: n.saturating_add(1), label: format!("SC{}", n) })
        }
        Some((a, b)) => {
            let (from, to) = (code(a)?, code(b)?);
            Some(Element { from, to, label: format!("SC{}-SC{}", from, to) })
        }
    }
}

/// The `disable=` elements of a `# shellcheck key=value ...` line.
fn scan_line(line: &str) -> Option<Directive> {
    let text = line.strip_suffix('\r').unwrap_or(line);
    let indent = text.len() - text.trim_start_matches(is_line_space).len();
    let rest = text[indent..].strip_prefix('#')?;
    let rest = rest.trim_start_matches(is_line_space).strip_prefix("shellcheck")?;
    if !rest.starts_with(is_line_space) {
        return None;
    }
    let mut rest = rest;
    let mut elements = vec![];
    loop {
        rest = rest.trim_start_matches(is_line_space);
        let key_len = rest.find(|c: char| !(c.is_ascii_alphabetic() || c == '-')).unwrap_or(rest.len());
        if key_len == 0 {
            break;
        }
        let key = &rest[..key_len];
        let Some(after) = rest[key_len..].strip_prefix('=') else { break };
        let (value, remaining) = match after.chars().next() {
            Some(q @ ('\'' | '"')) => match after[1..].find(q) {
                Some(end) => (&after[1..1 + end], &after[end + 2..]),
                None => break,
            },
            _ => {
                let end = if key == "disable" {
                    after.find(|c: char| !(c.is_ascii_alphanumeric() || c == ',' || c == '-')).unwrap_or(after.len())
                } else {
                    after.find(is_line_space).unwrap_or(after.len())
                };
                (&after[..end], &after[end..])
            }
        };
        if key == "disable" {
            elements.extend(value.split(',').filter_map(parse_element));
        }
        rest = remaining;
    }
    if elements.is_empty() {
        return None;
    }
    // Tabs count as 8, as in the parser's positions.
    let column_of = |upto: usize| {
        text[..upto].chars().fold(1i64, |col, c| if c == '\t' { col + 8 - ((col - 1) % 8) } else { col + 1 })
    };
    Some(Directive { line: 0, column: column_of(indent), end_column: column_of(text.len()), elements })
}

type Pos = (i64, i64);

fn pos_of(p: &Position) -> Pos {
    (p.line, p.column)
}

/// A `T_Annotation` with disables: the lines holding its directives and the
/// span of the code it applies to.
struct Scope {
    first_line: i64,
    last_line: i64,
    start: Pos,
    end: Pos,
}

/// The SC9006 reports for the `disable=` elements that suppressed nothing.
///
/// `honoured` and `ignored` are the unfiltered messages of the two runs and
/// `parsed` is the parse of the run that honoured directives.
pub fn unused_disable_reports(
    spec: &CheckSpec,
    contents: &str,
    parsed: &ParseResult,
    honoured: &[PositionedComment],
    ignored: &[PositionedComment],
) -> Vec<PositionedComment> {
    let Some(root) = parsed.root else { return vec![] };

    // Messages that only exist because directives were ignored.
    let key = |pc: &PositionedComment| {
        (pc.start_pos.file.clone(), pc.start_pos.line, pc.start_pos.column, pc.comment.code, pc.comment.message.clone())
    };
    let kept: HashSet<_> = honoured.iter().map(key).collect();
    let suppressed: Vec<&PositionedComment> = ignored.iter().filter(|pc| !kept.contains(&key(pc))).collect();

    // Where each in-file directive applies.
    let mut scopes: Vec<Scope> = vec![];
    let mut ids: Vec<Id> = vec![];
    parsed.ast.analyze(root, &mut |id| ids.push(id), &mut |_| {});
    for id in ids {
        let Token::T_Annotation(anns, child) = parsed.ast.get(id) else { continue };
        if !anns.iter().any(|a| matches!(a, shelllint_core::ast::Annotation::DisableComment(..))) {
            continue;
        }
        let (Some((a_start, a_end)), Some((c_start, c_end))) =
            (parsed.token_positions(id), parsed.token_positions(*child))
        else {
            continue;
        };
        if a_start.file != spec.filename || c_start.file != spec.filename {
            continue;
        }
        scopes.push(Scope {
            first_line: a_start.line,
            last_line: a_end.line,
            start: pos_of(&c_start),
            end: pos_of(&c_end),
        });
    }

    // Optional checks that are not enabled cannot be judged.
    let mut enabled = get_enable_directives(&parsed.ast, root);
    enabled.extend(spec.optional_checks.iter().cloned());
    let all_enabled = enabled.iter().any(|k| k == "all");
    let disabled_optional_codes: Vec<Code> = crate::optional_checks()
        .into_iter()
        .filter(|d| !all_enabled && !enabled.contains(&d.name))
        .flat_map(|d| d.id)
        .collect();

    let mut reports = vec![];
    for (index, line) in contents.split('\n').enumerate() {
        let line_no = index as i64 + 1;
        let Some(mut directive) = scan_line(line) else { continue };
        directive.line = line_no;
        // The innermost (last in pre-order) annotation holding this line.
        let Some(scope) = scopes.iter().rfind(|s| s.first_line <= line_no && line_no <= s.last_line) else {
            continue;
        };
        for element in &directive.elements {
            let used = suppressed.iter().any(|pc| {
                let code = pc.comment.code;
                pc.start_pos.file == spec.filename
                    && code >= element.from
                    && code < element.to
                    && scope.start <= pos_of(&pc.start_pos)
                    && pos_of(&pc.start_pos) <= scope.end
            });
            let unjudgeable = disabled_optional_codes.iter().any(|&c| c >= element.from && c < element.to);
            if used || unjudgeable {
                continue;
            }
            let position = |column: i64| Position { file: spec.filename.clone(), line: directive.line, column };
            reports.push(PositionedComment {
                start_pos: position(directive.column),
                end_pos: position(directive.end_column),
                comment: shelllint_core::interface::Comment {
                    severity: Severity::InfoC,
                    code: UNUSED_DISABLE_CODE,
                    message: format!("This disable directive suppresses nothing: {}", element.label),
                },
                ..new_positioned_comment()
            });
        }
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_elements() {
        let d =
            scan_line("  # shellcheck disable=SC2086,2034 source=x.sh disable=\"SC1000-SC1010,all\" # why").unwrap();
        let labels: Vec<&str> = d.elements.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["SC2086", "SC2034", "SC1000-SC1010", "all"]);
        assert_eq!(d.column, 3);
    }

    #[test]
    fn ignores_other_lines() {
        assert!(scan_line("echo # shellcheck disable=SC2086").is_none());
        assert!(scan_line("# shellcheck shell=bash").is_none());
        assert!(scan_line("# shellchecker disable=SC2086").is_none());
    }
}
