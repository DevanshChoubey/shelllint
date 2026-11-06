// SPDX-License-Identifier: GPL-3.0-or-later
//! Server settings, parsed from `initializationOptions` and
//! `workspace/didChangeConfiguration`.

use serde_json::Value;
use shellcheck_core::data::shell_for_executable;
use shellcheck_core::interface::{Code, Severity, Shell};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `None` means "auto": use the shebang / directive / file extension.
    pub dialect: Option<Shell>,
    pub enable: Vec<String>,
    pub exclude: Vec<Code>,
    pub min_severity: Severity,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { dialect: None, enable: vec![], exclude: vec![], min_severity: Severity::StyleC }
    }
}

impl Settings {
    /// Accepts a flat object (`{"dialect": ...}`) or one nested under
    /// `"shellcheck-rust"` / `"shellcheck"`. Unknown or malformed values are
    /// ignored, so a bad setting never stops the server.
    pub fn from_json(v: &Value) -> Settings {
        let mut s = Settings::default();
        let obj =
            ["shellcheck-rust", "shellcheck"].iter().find_map(|k| v.get(*k).filter(|x| x.is_object())).unwrap_or(v);
        if let Some(d) = obj.get("dialect").and_then(Value::as_str) {
            s.dialect = if d.is_empty() || d == "auto" { None } else { shell_for_executable(d) };
        }
        if let Some(list) = obj.get("enable").and_then(Value::as_array) {
            s.enable = list.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        }
        if let Some(list) = obj.get("exclude").and_then(Value::as_array) {
            s.exclude = list.iter().filter_map(parse_code).collect();
        }
        if let Some(sev) = obj.get("severity").and_then(Value::as_str) {
            s.min_severity = match sev {
                "error" => Severity::ErrorC,
                "warning" => Severity::WarningC,
                "info" => Severity::InfoC,
                _ => Severity::StyleC,
            };
        }
        s
    }
}

/// `SC2086`, `sc2086` or `2086` (number form also accepted).
pub fn parse_code(v: &Value) -> Option<Code> {
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    let s = v.as_str()?.trim();
    let digits = s.strip_prefix("SC").or_else(|| s.strip_prefix("sc")).unwrap_or(s);
    digits.parse().ok()
}
