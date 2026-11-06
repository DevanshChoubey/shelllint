// SPDX-License-Identifier: GPL-3.0-or-later
//! WebAssembly bindings for the browser playground (`playground/`).
//!
//! * `check(script, options_json)`: the `json1` report, as the CLI's `-f json1`.
//! * `fix(script, options_json, index)`: the script with the fix of diagnostic
//!   `index` (or of all fixes merged, as `-f diff` does, when `index < 0`).
//! * `list_checks()`: the optional checks (`--list-optional`) as JSON.
//! * `version()`: the ShellCheck version this port follows.
//!
//! `options_json` is an object; every key is optional:
//! `{"filename": "script.sh", "shell": "bash", "include": [2086],
//!   "exclude": [2034], "severity": "style", "enable": ["require-variable-braces"],
//!   "extended_analysis": true, "check_sourced": false}`.
//! Keys mirror the CLI flags (`--shell`, `--include`, `--exclude`,
//! `--severity`, `--enable`, `--extended-analysis`, `--sourced`). Unknown keys
//! are an error. `source`/`source-path` are not followed here: there is no
//! file system in the browser.

use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use shellcheck_checker::{check_script, optional_checks};
use shellcheck_core::data::{SHELLCHECK_VERSION, shell_for_executable};
use shellcheck_core::interface::{CheckResult, CheckSpec, Fix, Severity, empty_check_spec, new_system_interface};
use shellcheck_formatter::fixer::{apply_fix, merge_fixes};
use shellcheck_formatter::{Formatter, haskell_lines, json1};

/// The name the script is checked under when `filename` is not given.
/// It has no extension, so the shell is taken from the shebang only.
const DEFAULT_FILENAME: &str = "script.sh";

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Options {
    filename: Option<String>,
    shell: Option<String>,
    include: Vec<i64>,
    exclude: Vec<i64>,
    severity: Option<String>,
    enable: Vec<String>,
    extended_analysis: Option<bool>,
    check_sourced: bool,
}

fn parse_options(options_json: &str) -> Result<Options, String> {
    if options_json.trim().is_empty() {
        return Ok(Options::default());
    }
    serde_json::from_str(options_json).map_err(|e| format!("invalid options: {e}"))
}

/// Builds the `CheckSpec` the CLI builds from its flags (`parse_option`).
fn build_spec(script: &str, opts: &Options) -> Result<CheckSpec, String> {
    let mut spec = CheckSpec {
        filename: opts.filename.clone().unwrap_or_else(|| DEFAULT_FILENAME.to_string()),
        script: script.to_string(),
        ..empty_check_spec()
    };
    spec.shell_type_override = match opts.shell.as_deref() {
        None | Some("") => None,
        Some(name) => Some(shell_for_executable(name).ok_or_else(|| format!("Unknown shell: {name}"))?),
    };
    if let Some(sev) = opts.severity.as_deref() {
        spec.min_severity = match sev {
            "error" => Severity::ErrorC,
            "warning" => Severity::WarningC,
            "info" => Severity::InfoC,
            "style" => Severity::StyleC,
            other => {
                return Err(format!(
                    "Unknown value for severity: {other}. Valid options are: error, warning, info, style"
                ));
            }
        };
    }
    spec.excluded_warnings = opts.exclude.clone();
    // An empty `include` means "no include list", as in the CLI.
    spec.included_warnings = (!opts.include.is_empty()).then(|| opts.include.clone());
    spec.optional_checks = opts.enable.clone();
    spec.extended_analysis = opts.extended_analysis;
    spec.check_sourced = opts.check_sourced;
    Ok(spec)
}

fn lint(script: &str, options_json: &str) -> Result<CheckResult, String> {
    let spec = build_spec(script, &parse_options(options_json)?)?;
    Ok(check_script(&new_system_interface(), &spec))
}

/// `-f json1` for one file: the same bytes the CLI prints.
fn render_json1(result: &CheckResult, source: &str) -> String {
    let mut formatter = json1::format();
    let mut out: Vec<u8> = Vec::new();
    formatter.header(&mut out);
    // The formatter re-reads the checked file's contents, as the CLI does.
    let sys = shellcheck_core::interface::mocked_system_interface(vec![(result.filename.clone(), source.to_string())]);
    formatter.on_result(result, &sys, &mut out);
    formatter.footer(&mut out);
    String::from_utf8(out).unwrap_or_default()
}

/// Checks `script`. Returns the `json1` report (`{"comments":[...]}`).
/// Throws (`Err`) only for malformed options.
#[wasm_bindgen]
pub fn check(script: &str, options_json: &str) -> Result<String, String> {
    let result = lint(script, options_json)?;
    Ok(render_json1(&result, script))
}

/// Applies the fix of diagnostic `index` (the position in `check`'s
/// `comments`), or every fix merged when `index` is negative. Fixes that
/// overlap an earlier one are skipped, as in `-f diff`.
#[wasm_bindgen]
pub fn fix(script: &str, options_json: &str, index: i32) -> Result<String, String> {
    let result = lint(script, options_json)?;
    let fixes: Vec<Fix> = if index < 0 {
        result.comments.iter().filter_map(|c| c.fix.clone()).collect()
    } else {
        let comment = result.comments.get(index as usize).ok_or_else(|| format!("no diagnostic at index {index}"))?;
        comment.fix.iter().cloned().collect()
    };
    if fixes.is_empty() {
        return Ok(script.to_string());
    }
    let lines = haskell_lines(script);
    let mut out = apply_fix(&merge_fixes(&fixes), &lines).join("\n");
    if script.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// The optional checks (`--list-optional`): name, codes, description and an
/// example, as a JSON array.
#[wasm_bindgen]
pub fn list_checks() -> String {
    let list: Vec<Value> = optional_checks()
        .into_iter()
        .map(|c| {
            json!({
                "name": c.name,
                "codes": c.id,
                "description": c.description,
                "positive": c.positive,
                "negative": c.negative,
            })
        })
        .collect();
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
}

/// The ShellCheck version this port follows, e.g. `0.11.0`.
#[wasm_bindgen]
pub fn version() -> String {
    SHELLCHECK_VERSION.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shebang, so SC2148 (no shell declared) is not reported.
    const SCRIPT: &str = "#!/bin/sh\necho $1\n";

    fn codes(json: &str) -> Vec<i64> {
        let v: Value = serde_json::from_str(json).unwrap();
        v["comments"].as_array().unwrap().iter().map(|c| c["code"].as_i64().unwrap()).collect()
    }

    #[test]
    fn reports_unquoted_variable() {
        assert_eq!(codes(&check(SCRIPT, "").unwrap()), vec![2086]);
    }

    #[test]
    fn exclude_and_include() {
        assert!(codes(&check(SCRIPT, r#"{"exclude":[2086]}"#).unwrap()).is_empty());
        assert_eq!(codes(&check(SCRIPT, r#"{"include":[2086]}"#).unwrap()), vec![2086]);
        assert!(codes(&check(SCRIPT, r#"{"include":[2148]}"#).unwrap()).is_empty());
    }

    #[test]
    fn severity_filters() {
        let all = codes(&check(SCRIPT, "").unwrap());
        assert!(!all.is_empty());
        assert!(codes(&check(SCRIPT, r#"{"severity":"error"}"#).unwrap()).is_empty());
    }

    #[test]
    fn bad_options_are_errors() {
        assert!(check("", r#"{"nope":1}"#).is_err());
        assert!(check("", r#"{"shell":"tcsh"}"#).is_err());
        assert!(check("", r#"{"severity":"loud"}"#).is_err());
    }

    #[test]
    fn fix_quotes_variable() {
        assert_eq!(fix(SCRIPT, "", 0).unwrap(), "#!/bin/sh\necho \"$1\"\n");
        assert_eq!(fix(SCRIPT, "", -1).unwrap(), "#!/bin/sh\necho \"$1\"\n");
        assert!(fix(SCRIPT, "", 5).is_err());
    }

    #[test]
    fn lists_optional_checks() {
        let v: Value = serde_json::from_str(&list_checks()).unwrap();
        assert!(v.as_array().map_or(false, |a| !a.is_empty()));
    }
}
