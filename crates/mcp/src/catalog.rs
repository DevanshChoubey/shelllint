// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything the tools know about a ShellCheck code: the generated static
//! catalogue (`data/codes.json`, see tools/gen_mcp_codes.py) merged with the
//! optional checks' descriptions read from the analyzer at run time.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;
use shellcheck_core::interface::CheckDescription;

const CODES_JSON: &str = include_str!("../data/codes.json");

#[derive(Clone, Debug)]
pub struct Optional {
    /// The name passed to `enable`.
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub code: u32,
    /// Haskell check name that emits the code.
    pub check: Option<String>,
    /// error | warning | info | style
    pub severity: Option<String>,
    pub message: Option<String>,
    pub bad: Option<String>,
    pub good: Option<String>,
    /// Set for checks that are off unless enabled.
    pub optional: Option<Optional>,
}

static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();

/// Every known code, sorted by number.
pub fn entries() -> &'static [Entry] {
    ENTRIES.get_or_init(build)
}

pub fn find(code: u32) -> Option<&'static Entry> {
    entries().binary_search_by_key(&code, |e| e.code).ok().map(|i| &entries()[i])
}

/// Names accepted by `enable`, sorted.
pub fn optional_names() -> Vec<String> {
    let mut names: Vec<String> = entries().iter().filter_map(|e| e.optional.as_ref().map(|o| o.name.clone())).collect();
    names.sort();
    names.dedup();
    names
}

/// Accepts "SC2086", "sc2086" or "2086".
pub fn parse_code(s: &str) -> Option<u32> {
    let t = s.trim();
    let digits = if t.len() >= 2 && t[..2].eq_ignore_ascii_case("sc") { &t[2..] } else { t };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from)
}

fn build() -> Vec<Entry> {
    let mut map: BTreeMap<u32, Entry> = BTreeMap::new();
    let rows: Vec<Value> = serde_json::from_str(CODES_JSON).expect("data/codes.json is valid JSON");
    for row in rows {
        let Some(code) = row.get("code").and_then(Value::as_u64).and_then(|c| u32::try_from(c).ok()) else {
            continue;
        };
        map.insert(
            code,
            Entry {
                code,
                check: text(&row, "check"),
                severity: text(&row, "severity"),
                message: text(&row, "message"),
                bad: text(&row, "bad"),
                good: text(&row, "good"),
                optional: None,
            },
        );
    }
    let optional: Vec<CheckDescription> = shellcheck_analyzer::api::analytics_optional_checks()
        .into_iter()
        .chain(shellcheck_analyzer::api::commands_optional_checks())
        .collect();
    for d in optional {
        for &code in &d.id {
            let code = u32::try_from(code).expect("SC codes are positive");
            let e = map.entry(code).or_insert_with(|| Entry {
                code,
                check: None,
                severity: None,
                message: None,
                bad: None,
                good: None,
                optional: None,
            });
            // The optional check's own examples are the documentation: prefer them.
            if !d.positive.is_empty() {
                e.bad = Some(d.positive.clone());
            }
            if !d.negative.is_empty() {
                e.good = Some(d.negative.clone());
            }
            e.optional = Some(Optional { name: d.name.clone(), description: d.description.clone() });
        }
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_accepts_common_spellings() {
        assert_eq!(parse_code("SC2086"), Some(2086));
        assert_eq!(parse_code("sc2086"), Some(2086));
        assert_eq!(parse_code("2086"), Some(2086));
        assert_eq!(parse_code("SC"), None);
        assert_eq!(parse_code("foo"), None);
    }

    #[test]
    fn catalogue_has_sc2086_with_example() {
        let e = find(2086).expect("SC2086 is in the catalogue");
        assert_eq!(e.severity.as_deref(), Some("info"));
        assert!(e.bad.is_some());
    }
}
