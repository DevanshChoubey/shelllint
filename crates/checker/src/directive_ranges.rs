// SPDX-License-Identifier: GPL-3.0-or-later
// shelllint extension (no upstream counterpart): line-range directives.
//
//   # shellcheck disable-start=SC2086,SC2034   suppress from this line on
//   # shellcheck disable-end=SC2086            end the range (or end of file)
//   cmd $x # shellcheck disable-line=SC2086    suppress on this line only
//
// `all` is accepted wherever a code list is. The directives are found by a
// text scan, so the existing `disable=` scoping is untouched. Only comments
// that start a line (start/end/line) or follow whitespace outside quotes on a
// command line (line only) count.
// DIVERGENCE: whole feature; upstream has no range directives.

use shelllint_core::interface::PositionedComment;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    All,
    Code(i64),
}

#[derive(Debug, Default)]
pub struct DirectiveRanges {
    /// (target, first line, last line), lines inclusive and 1 based.
    ranges: Vec<(Target, i64, i64)>,
}

fn parse_targets(value: &str) -> Vec<Target> {
    let value = value.trim_matches(|c| c == '"' || c == '\'');
    value
        .split(',')
        .filter_map(|item| {
            let item = item.trim();
            if item == "all" {
                return Some(Target::All);
            }
            item.strip_prefix("SC").unwrap_or(item).parse::<i64>().ok().map(Target::Code)
        })
        .collect()
}

/// `(key, targets)` pairs of a directive comment body (text after the `#`).
fn parse_directive(body: &str) -> Vec<(String, Vec<Target>)> {
    let rest = body.trim_start();
    let Some(rest) = rest.strip_prefix("shellcheck") else { return vec![] };
    if !rest.starts_with([' ', '\t']) {
        return vec![];
    }
    let mut out = vec![];
    for token in rest.split_whitespace() {
        if token.starts_with('#') {
            break;
        }
        if let Some((key, value)) = token.split_once('=') {
            if matches!(key, "disable-start" | "disable-end" | "disable-line") {
                out.push((key.to_string(), parse_targets(value)));
            }
        }
    }
    out
}

/// Byte index of a `#` that starts a trailing comment on a command line.
fn trailing_comment(line: &str) -> Option<usize> {
    let (mut single, mut double, mut escaped) = (false, false, false);
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' && !single {
            escaped = true;
        } else if c == '\'' && !double {
            single = !single;
        } else if c == '"' && !single {
            double = !double;
        } else if c == '#' && !single && !double && prev_space {
            return Some(i);
        }
        prev_space = c == ' ' || c == '\t';
    }
    None
}

impl DirectiveRanges {
    /// `None` when the script mentions no range directive at all.
    pub fn scan(contents: &str) -> Option<DirectiveRanges> {
        if !contents.contains("disable-start")
            && !contents.contains("disable-end")
            && !contents.contains("disable-line")
        {
            return None;
        }
        let mut ranges = vec![];
        let mut open: Vec<(Target, i64)> = vec![];
        let mut last = 0;
        for (idx, line) in contents.lines().enumerate() {
            let n = idx as i64 + 1;
            last = n;
            let Some(pos) = trailing_comment(line) else { continue };
            let standalone = line[..pos].trim().is_empty();
            for (key, targets) in parse_directive(&line[pos + 1..]) {
                match key.as_str() {
                    "disable-line" => ranges.extend(targets.into_iter().map(|t| (t, n, n))),
                    _ if !standalone => {}
                    "disable-start" => {
                        for t in targets {
                            if !open.iter().any(|(o, _)| *o == t) {
                                open.push((t, n));
                            }
                        }
                    }
                    _ => {
                        for t in targets {
                            let closing = |o: &Target| if t == Target::All { true } else { *o == t };
                            open.retain(|(o, from)| {
                                if closing(o) {
                                    ranges.push((*o, *from, n));
                                    false
                                } else {
                                    true
                                }
                            });
                        }
                    }
                }
            }
        }
        // Unterminated ranges run to the end of the file.
        ranges.extend(open.into_iter().map(|(t, from)| (t, from, last.max(from))));
        if ranges.is_empty() { None } else { Some(DirectiveRanges { ranges }) }
    }

    /// Is this message (in the checked file, not a sourced one) suppressed?
    pub fn suppresses(&self, filename: &str, pc: &PositionedComment) -> bool {
        if pc.start_pos.file != filename {
            return false;
        }
        let line = pc.start_pos.line;
        let code = pc.comment.code;
        self.ranges.iter().any(|(t, from, to)| {
            line >= *from && line <= *to && matches!(t, Target::All) | matches!(t, Target::Code(c) if *c == code)
        })
    }
}
