// SPDX-License-Identifier: GPL-3.0-or-later
//! The MCP tools: definitions for `tools/list` and handlers for `tools/call`.
//! Handlers never panic on bad input: problems come back as `isError` results
//! so the calling model can read and correct them.

use std::fmt::Write as _;

use serde_json::{Value, json};
use shellcheck_checker::{check_script, decode_string};
use shellcheck_core::data::shell_for_executable;
use shellcheck_core::interface::{
    CheckSpec, Code, PositionedComment, Severity, Shell, empty_check_spec,
};
use shellcheck_formatter::diff::{Diff, get_diff};
use shellcheck_formatter::fixer::{apply_fix, merge_fixes};
use shellcheck_formatter::tty::WIKI_LINK;
use shellcheck_formatter::{
    code_no, col_no, end_col_no, end_line_no, haskell_lines, line_no, make_non_virtual, message_text, severity_text,
};

use crate::catalog;
use crate::sys::McpSystem;

/// Largest script accepted, in bytes.
pub const MAX_BYTES: usize = 1024 * 1024;
/// Most diagnostics written into the text summary.
const MAX_TEXT_LINES: usize = 50;
const DEFAULT_NAME: &str = "script.sh";

/// Result of a tool call, before it is wrapped as an MCP `CallToolResult`.
pub struct ToolOutput {
    pub text: String,
    pub structured: Option<Value>,
    pub is_error: bool,
}

impl ToolOutput {
    fn ok(text: String, structured: Value) -> Self {
        ToolOutput { text, structured: Some(structured), is_error: false }
    }
    pub fn error(text: impl Into<String>) -> Self {
        ToolOutput { text: text.into(), structured: None, is_error: true }
    }
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "content": [{"type": "text", "text": self.text}],
            "isError": self.is_error,
            "resultType": "complete",
        });
        if let Some(s) = &self.structured {
            v["structuredContent"] = s.clone();
        }
        v
    }
}

const SHELL_OPTIONS: &[&str] = &["sh", "bash", "dash", "ksh", "busybox"];
const SEVERITY_OPTIONS: &[&str] = &["error", "warning", "info", "style"];

fn lint_props() -> Value {
    json!({
        "shell": {
            "type": "string",
            "enum": SHELL_OPTIONS,
            "description": "Dialect to check against. Leave out to use the shebang or the file name; set it when the shebang is missing or wrong."
        },
        "enable": {
            "type": "array",
            "items": {"type": "string"},
            "description": "Optional checks to turn on, by name (see list_checks with optional=true), or [\"all\"]."
        },
        "exclude": {
            "type": "array",
            "items": {"type": "string"},
            "description": "Codes to silence, e.g. [\"SC2086\"]."
        },
        "severity": {
            "type": "string",
            "enum": SEVERITY_OPTIONS,
            "description": "Lowest severity to report. Default: style (report everything)."
        }
    })
}

fn with_lint_props(mut props: Value, extra: Value) -> Value {
    if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            p.insert(k.clone(), v.clone());
        }
    }
    props
}

/// `tools/list` payload.
pub fn definitions() -> Value {
    json!([
        {
            "name": "check_script",
            "title": "Lint shell script text",
            "description": "Lint a shell script with ShellCheck and list its problems: SC code, severity, line and column, message, and whether an automatic fix exists. Use it on script text before you write or finish editing a shell script. Pass the whole script in `script`. Set `shell` when the shebang is missing or wrong. Use `enable` for optional checks, `exclude` to silence codes.",
            "inputSchema": {
                "type": "object",
                "properties": with_lint_props(
                    json!({"script": {"type": "string", "description": "The full shell script text."}}),
                    lint_props(),
                ),
                "required": ["script"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "check_file",
            "title": "Lint a shell script file",
            "description": "Lint a shell script saved on disk, given its path. Same output as check_script. Use it when the script is already a file. Relative paths resolve against the server's working directory, so prefer absolute paths. Files over 1 MiB are rejected.",
            "inputSchema": {
                "type": "object",
                "properties": with_lint_props(
                    json!({"path": {"type": "string", "description": "Path to the script file."}}),
                    lint_props(),
                ),
                "required": ["path"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "explain",
            "title": "Explain a ShellCheck code",
            "description": "Explain one ShellCheck code such as SC2086: what the check reports, the check's name, its severity, a wiki link, and a bad and a good example. Use it to understand a diagnostic from check_script before you fix it, or to decide whether an exclusion is justified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {"type": "string", "description": "The code, e.g. \"SC2086\" or \"2086\"."}
                },
                "required": ["code"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "list_checks",
            "title": "List ShellCheck checks",
            "description": "List the ShellCheck checks with their codes and messages. Optional checks are off unless enabled. Use `filter` (matches code, check name or message) to narrow the list, and `optional: true` to list only the checks that must be turned on with `enable`. Use it to find codes or names before calling check_script.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": {"type": "string", "description": "Case-insensitive text to match in the code, check name or message."},
                    "optional": {"type": "boolean", "description": "true: only optional checks (off by default). false or omitted: all checks."}
                },
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "fix_script",
            "title": "Apply ShellCheck auto-fixes",
            "description": "Apply every automatic fix ShellCheck offers to a script. Returns the fixed script, a unified diff, how many fixes were applied, and the problems left over after fixing (those need a manual change). Use it when you want a corrected version of a script. Takes the same options as check_script.",
            "inputSchema": {
                "type": "object",
                "properties": with_lint_props(
                    json!({"script": {"type": "string", "description": "The full shell script text."}}),
                    lint_props(),
                ),
                "required": ["script"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false}
        }
    ])
}

pub fn exists(name: &str) -> bool {
    matches!(name, "check_script" | "check_file" | "explain" | "list_checks" | "fix_script")
}

/// Run a tool. `name` must satisfy `exists`.
pub fn call(name: &str, args: &Value) -> ToolOutput {
    match name {
        "check_script" => match required_str(args, "script") {
            Ok(script) => check_text(script, DEFAULT_NAME, false, args),
            Err(e) => ToolOutput::error(e),
        },
        "check_file" => match required_str(args, "path") {
            Ok(path) => check_path(path, args),
            Err(e) => ToolOutput::error(e),
        },
        "explain" => match required_str(args, "code") {
            Ok(code) => explain(code),
            Err(e) => ToolOutput::error(e),
        },
        "list_checks" => list_checks(args),
        "fix_script" => match required_str(args, "script") {
            Ok(script) => fix_text(script, args),
            Err(e) => ToolOutput::error(e),
        },
        _ => ToolOutput::error(format!("Unknown tool: {name}")),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| format!("Missing required string argument '{key}'"))
}

// ---- options -----------------------------------------------------------------

struct LintOptions {
    shell: Option<Shell>,
    enable: Vec<String>,
    exclude: Vec<Code>,
    min_severity: Severity,
}

fn parse_options(args: &Value) -> Result<LintOptions, String> {
    let shell = match args.get("shell").and_then(Value::as_str) {
        None => None,
        Some(s) => Some(
            shell_for_executable(s)
                .ok_or_else(|| format!("Unknown shell '{s}'. Valid values: {}", SHELL_OPTIONS.join(", ")))?,
        ),
    };

    let mut enable = Vec::new();
    let known = catalog::optional_names();
    for name in string_list(args, "enable")? {
        if name == "all" {
            enable.push(name);
        } else if known.contains(&name) {
            enable.push(name);
        } else {
            return Err(format!("Unknown optional check '{name}'. Valid names: all, {}", known.join(", ")));
        }
    }

    let mut exclude = Vec::new();
    for s in string_list(args, "exclude")? {
        let code =
            catalog::parse_code(&s).ok_or_else(|| format!("'{s}' is not a ShellCheck code (expected e.g. SC2086)"))?;
        exclude.push(Code::from(code));
    }

    let min_severity = match args.get("severity").and_then(Value::as_str).unwrap_or("style") {
        "error" => Severity::ErrorC,
        "warning" => Severity::WarningC,
        "info" => Severity::InfoC,
        "style" => Severity::StyleC,
        other => return Err(format!("Unknown severity '{other}'. Valid values: {}", SEVERITY_OPTIONS.join(", "))),
    };

    Ok(LintOptions { shell, enable, exclude, min_severity })
}

/// An array of strings, or a comma-separated string.
fn string_list(args: &Value, key: &str) -> Result<Vec<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::String(s)) => Ok(split_commas(s)),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str().map(|s| s.trim().to_string()).ok_or_else(|| format!("'{key}' must contain only strings"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.into_iter().filter(|s| !s.is_empty()).collect()),
        Some(_) => Err(format!("'{key}' must be an array of strings")),
    }
}

fn split_commas(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

// ---- linting -----------------------------------------------------------------

/// Run the full pipeline. The comments keep the checker's own (tab-stop 8)
/// positions, which `apply_fix` needs. `is_file`: `filename` is the file on
/// disk the script came from, so it counts as an input (as on the CLI).
fn lint(script: &str, filename: &str, is_file: bool, opts: &LintOptions) -> Vec<PositionedComment> {
    let spec = CheckSpec {
        filename: filename.to_string(),
        script: script.to_string(),
        ignore_rc: true,
        shell_type_override: opts.shell,
        excluded_warnings: opts.exclude.clone(),
        min_severity: opts.min_severity,
        optional_checks: opts.enable.clone(),
        ..empty_check_spec()
    };
    let inputs: &[&str] = if is_file { &[filename] } else { &[] };
    let sys = McpSystem::new(inputs);
    check_script(&sys, &spec).comments
}

fn check_text(script: &str, filename: &str, is_file: bool, args: &Value) -> ToolOutput {
    if script.len() > MAX_BYTES {
        return ToolOutput::error("Script is larger than 1 MiB; split it or lint the file with check_file.");
    }
    let opts = match parse_options(args) {
        Ok(o) => o,
        Err(e) => return ToolOutput::error(e),
    };
    let comments = lint(script, filename, is_file, &opts);
    report(filename, script, &comments)
}

fn check_path(path: &str, args: &Value) -> ToolOutput {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error(format!("Cannot read {path}: {e}")),
    };
    if !meta.is_file() {
        return ToolOutput::error(format!("{path} is not a regular file"));
    }
    if meta.len() as usize > MAX_BYTES {
        return ToolOutput::error(format!("{path} is larger than 1 MiB; refusing to lint it"));
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return ToolOutput::error(format!("Cannot read {path}: {e}")),
    };
    let script = decode_string(&bytes);
    check_text(&script, path, true, args)
}

/// Build the diagnostics output for a lint run.
fn report(filename: &str, script: &str, raw: &[PositionedComment]) -> ToolOutput {
    let comments = make_non_virtual(raw, script);
    let fixable = comments.iter().filter(|c| c.fix.is_some()).count();
    let diagnostics: Vec<Value> = comments.iter().map(diagnostic_json).collect();

    let mut text = String::new();
    if comments.is_empty() {
        let _ = write!(text, "{filename}: no issues found ({} lines checked).", script.lines().count());
    } else {
        let counts = severity_counts(&comments);
        let _ = writeln!(
            text,
            "{filename}: {} issue{} ({}). {fixable} auto-fixable; fix_script applies them.",
            comments.len(),
            plural(comments.len()),
            counts
        );
        for c in comments.iter().take(MAX_TEXT_LINES) {
            let mark = if c.fix.is_some() { " [auto-fix]" } else { "" };
            let _ = writeln!(
                text,
                "line {}, col {}: SC{} [{}] {}{mark}",
                line_no(c),
                col_no(c),
                code_no(c),
                severity_text(c),
                message_text(c)
            );
        }
        if comments.len() > MAX_TEXT_LINES {
            let _ = writeln!(text, "... and {} more", comments.len() - MAX_TEXT_LINES);
        }
        text.truncate(text.trim_end().len());
    }

    let structured = json!({
        "file": filename,
        "count": comments.len(),
        "fixable": fixable,
        "diagnostics": diagnostics,
    });
    ToolOutput::ok(text, structured)
}

fn diagnostic_json(c: &PositionedComment) -> Value {
    let fix = c.fix.as_ref().map(|f| {
        json!({
            "replacements": f.replacements.iter().map(|r| json!({
                "line": r.start_pos.line,
                "column": r.start_pos.column,
                "endLine": r.end_pos.line,
                "endColumn": r.end_pos.column,
                "text": r.string,
            })).collect::<Vec<_>>()
        })
    });
    let code = code_no(c);
    json!({
        "code": format!("SC{code}"),
        "number": code,
        "severity": severity_text(c),
        "line": line_no(c),
        "column": col_no(c),
        "endLine": end_line_no(c),
        "endColumn": end_col_no(c),
        "message": message_text(c),
        "wiki": format!("{WIKI_LINK}SC{code}"),
        "fix": fix,
    })
}

fn severity_counts(comments: &[PositionedComment]) -> String {
    let mut parts = Vec::new();
    for sev in ["error", "warning", "info", "style"] {
        let n = comments.iter().filter(|c| severity_text(c) == sev).count();
        if n > 0 {
            parts.push(format!("{n} {sev}{}", plural(n)));
        }
    }
    parts.join(", ")
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// ---- fixing ------------------------------------------------------------------

fn fix_text(script: &str, args: &Value) -> ToolOutput {
    if script.len() > MAX_BYTES {
        return ToolOutput::error("Script is larger than 1 MiB; split it first.");
    }
    let opts = match parse_options(args) {
        Ok(o) => o,
        Err(e) => return ToolOutput::error(e),
    };
    let raw = lint(script, DEFAULT_NAME, false, &opts);
    let fixes: Vec<_> = raw.iter().filter_map(|c| c.fix.clone()).collect();
    let applied = fixes.len();

    let old_lines: Vec<String> = haskell_lines(script).into_iter().map(String::from).collect();
    let fixed = if fixes.is_empty() {
        script.to_string()
    } else {
        let refs: Vec<&str> = old_lines.iter().map(String::as_str).collect();
        let new_lines = apply_fix(&merge_fixes(&fixes), &refs);
        let mut out = new_lines.join("\n");
        if script.ends_with('\n') && !new_lines.is_empty() {
            out.push('\n');
        }
        out
    };

    // Re-check the fixed script so `remaining` reflects what is left after the fixes.
    let remaining = if fixes.is_empty() { raw.clone() } else { lint(&fixed, DEFAULT_NAME, false, &opts) };
    let remaining_comments = make_non_virtual(&remaining, &fixed);
    let diff = unified_diff(
        DEFAULT_NAME,
        &old_lines,
        &haskell_lines(&fixed).into_iter().map(String::from).collect::<Vec<_>>(),
    );

    let mut text = if applied == 0 {
        "No automatic fixes apply to this script.".to_string()
    } else {
        format!("Applied {applied} fix{}.", if applied == 1 { "" } else { "es" })
    };
    text.push_str(&format!(
        " {} issue{} remain{}.",
        remaining_comments.len(),
        plural(remaining_comments.len()),
        if remaining_comments.len() == 1 { "s" } else { "" }
    ));
    if !diff.is_empty() {
        text.push_str(&format!("\n\n```diff\n{diff}```"));
    }
    text.push_str(&format!("\n\nFixed script:\n```sh\n{fixed}```"));

    let structured = json!({
        "fixed_script": fixed,
        "diff": diff,
        "applied": applied,
        "remaining": remaining_comments.iter().map(diagnostic_json).collect::<Vec<_>>(),
    });
    ToolOutput::ok(text, structured)
}

/// Unified diff with three lines of context; empty when nothing changed.
pub fn unified_diff(name: &str, old: &[String], new: &[String]) -> String {
    const CONTEXT: usize = 3;
    // Each op: (tag, text). Tags: ' ', '-', '+'.
    let mut ops: Vec<(char, &str)> = Vec::new();
    let diffs = get_diff(old, new);
    for d in &diffs {
        match d {
            Diff::Both(a, _) => ops.push((' ', a.as_str())),
            Diff::First(a) => ops.push(('-', a.as_str())),
            Diff::Second(b) => ops.push(('+', b.as_str())),
        }
    }
    let changes: Vec<usize> = ops.iter().enumerate().filter(|(_, (t, _))| *t != ' ').map(|(i, _)| i).collect();
    if changes.is_empty() {
        return String::new();
    }

    // Group changes whose contexts overlap into hunks.
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for &i in &changes {
        let start = i.saturating_sub(CONTEXT);
        let end = (i + CONTEXT + 1).min(ops.len());
        match hunks.last_mut() {
            Some(last) if start <= last.1 => last.1 = end.max(last.1),
            _ => hunks.push((start, end)),
        }
    }

    let mut out = format!("--- a/{name}\n+++ b/{name}\n");
    for (start, end) in hunks {
        let before_old = ops[..start].iter().filter(|(t, _)| *t != '+').count();
        let before_new = ops[..start].iter().filter(|(t, _)| *t != '-').count();
        let slice = &ops[start..end];
        let count_old = slice.iter().filter(|(t, _)| *t != '+').count();
        let count_new = slice.iter().filter(|(t, _)| *t != '-').count();
        let line_old = if count_old == 0 { before_old } else { before_old + 1 };
        let line_new = if count_new == 0 { before_new } else { before_new + 1 };
        let _ = writeln!(out, "@@ -{line_old},{count_old} +{line_new},{count_new} @@");
        for (t, text) in slice {
            let _ = writeln!(out, "{t}{text}");
        }
    }
    out
}

// ---- explain / list ----------------------------------------------------------

fn explain(code: &str) -> ToolOutput {
    let Some(number) = catalog::parse_code(code) else {
        return ToolOutput::error(format!(
            "'{code}' is not a ShellCheck code (expected e.g. SC2086). Use list_checks to find codes."
        ));
    };
    let Some(e) = catalog::find(number) else {
        return ToolOutput::error(format!(
            "SC{number} is not a code this server knows. Use list_checks to see the known codes."
        ));
    };
    let wiki = format!("{WIKI_LINK}SC{number}");
    let severity = e.severity.as_deref().unwrap_or("unknown");

    let mut text = format!("SC{number} ({severity})\n");
    if let Some(m) = &e.message {
        text.push_str(&format!("Message: {m}\n"));
    }
    if let Some(c) = &e.check {
        text.push_str(&format!("Check: {c}\n"));
    }
    if let Some(o) = &e.optional {
        text.push_str(&format!(
            "Optional check '{}': off by default; enable it with enable=[\"{}\"].\n",
            o.name, o.name
        ));
        text.push_str(&format!("Description: {}\n", o.description));
    }
    text.push_str(&format!("Wiki: {wiki}\n"));
    if let Some(b) = &e.bad {
        text.push_str(&format!("\nBad example (reports SC{number}):\n{}\n", indent(b)));
    }
    if let Some(g) = &e.good {
        text.push_str(&format!("\nGood example:\n{}\n", indent(g)));
    }

    let structured = json!({
        "code": format!("SC{number}"),
        "number": number,
        "severity": e.severity,
        "check": e.check,
        "message": e.message,
        "wiki": wiki,
        "optional": e.optional.as_ref().map(|o| json!({"name": o.name, "description": o.description})),
        "bad": e.bad,
        "good": e.good,
    });
    ToolOutput::ok(text.trim_end().to_string(), structured)
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("    {l}")).collect::<Vec<_>>().join("\n")
}

const LIST_LIMIT: usize = 100;

fn list_checks(args: &Value) -> ToolOutput {
    let filter = args.get("filter").and_then(Value::as_str).map(str::to_lowercase).filter(|s| !s.is_empty());
    let only_optional = args.get("optional").and_then(Value::as_bool).unwrap_or(false);

    let matches: Vec<&catalog::Entry> = catalog::entries()
        .iter()
        .filter(|e| !only_optional || e.optional.is_some())
        .filter(|e| match &filter {
            None => true,
            Some(f) => {
                format!("sc{}", e.code).contains(f)
                    || e.check.as_deref().is_some_and(|c| c.to_lowercase().contains(f))
                    || e.message.as_deref().is_some_and(|m| m.to_lowercase().contains(f))
                    || e.optional.as_ref().is_some_and(|o| o.name.contains(f))
            }
        })
        .collect();

    if matches.is_empty() {
        return ToolOutput::ok("No checks match.".to_string(), json!({"count": 0, "checks": []}));
    }

    let mut text = String::new();
    for e in matches.iter().take(LIST_LIMIT) {
        let sev = e.severity.as_deref().unwrap_or("unknown");
        let msg = e.message.as_deref().or(e.optional.as_ref().map(|o| o.description.as_str())).unwrap_or("");
        let opt = e.optional.as_ref().map(|o| format!(" (optional: enable \"{}\")", o.name)).unwrap_or_default();
        let _ = writeln!(text, "SC{} [{sev}] {msg}{opt}", e.code);
    }
    if matches.len() > LIST_LIMIT {
        let _ = writeln!(text, "... showing {LIST_LIMIT} of {}. Add a filter to narrow the list.", matches.len());
    }
    let checks: Vec<Value> = matches
        .iter()
        .map(|e| {
            json!({
                "code": format!("SC{}", e.code),
                "number": e.code,
                "check": e.check,
                "severity": e.severity,
                "message": e.message,
                "optional": e.optional.as_ref().map(|o| o.name.clone()),
            })
        })
        .collect();
    ToolOutput::ok(text.trim_end().to_string(), json!({"count": matches.len(), "checks": checks}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_shows_one_hunk_with_context() {
        let old: Vec<String> = ["a", "b", "c", "d", "e", "f", "g", "h"].iter().map(|s| s.to_string()).collect();
        let mut new = old.clone();
        new[3] = "D".to_string();
        let d = unified_diff("x.sh", &old, &new);
        assert_eq!(d, "--- a/x.sh\n+++ b/x.sh\n@@ -1,7 +1,7 @@\n a\n b\n c\n-d\n+D\n e\n f\n g\n");
    }

    #[test]
    fn unified_diff_is_empty_when_unchanged() {
        let old = vec!["x".to_string()];
        assert_eq!(unified_diff("x.sh", &old, &old), "");
    }
}
