// SPDX-License-Identifier: GPL-3.0-or-later
//! The `SystemInterface` the server checks with. Config discovery mirrors the
//! CLI (`crates/cli/src/sys.rs`): `.shellcheckrc` / `shellcheckrc` walking up
//! from the script, then the home and XDG config files, merged with
//! `.editorconfig` through `shellcheck_checker::editorconfig`.
//! Files are read from disk on every check, so rc edits apply on the next run.

use std::path::{Path, PathBuf};

use shellcheck_checker::editorconfig::{EditorConfigFs, get_editor_config, merge_configs};
use shellcheck_core::interface::SystemInterface;

#[derive(Default)]
pub struct LspSystem;

fn dir_of(path: &str) -> String {
    match Path::new(path).parent() {
        Some(p) if p.as_os_str().is_empty() => ".".to_string(),
        Some(p) => p.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

fn read_config(file: &str) -> Option<(String, String)> {
    if !Path::new(file).is_file() {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    Some((file.to_string(), String::from_utf8_lossy(&bytes).into_owned()))
}

fn xdg_dir() -> Option<String> {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| home.map(|h| format!("{}/.config", h)))
}

/// Candidate rc paths for `filename`: nearest directory first, then the
/// user-level files.
fn rc_candidates(filename: &str) -> Vec<String> {
    let mut out = vec![];
    let mut dir = dir_of(filename);
    loop {
        let join = |name: &str| {
            if dir.ends_with('/') { format!("{}{}", dir, name) } else { format!("{}/{}", dir, name) }
        };
        out.push(join(".shellcheckrc"));
        out.push(join("shellcheckrc"));
        let next = dir_of(&dir);
        if next == dir {
            break;
        }
        dir = next;
    }
    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
        out.push(format!("{}/.shellcheckrc", home));
    }
    if let Some(x) = xdg_dir() {
        out.push(format!("{}/shellcheckrc", x));
    }
    out
}

impl EditorConfigFs for LspSystem {
    fn normalize(&self, path: &str) -> String {
        std::fs::canonicalize(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string())
    }

    fn read_config(&self, file: &str) -> Option<(String, String)> {
        read_config(file)
    }

    fn global_editor_config_path(&self) -> Option<String> {
        Some(format!("{}/editorconfig.ini", xdg_dir()?))
    }
}

impl SystemInterface for LspSystem {
    /// Only the file being edited is an input; sourcing other files needs the
    /// rc to say `external-sources`, as on the CLI.
    fn read_file(&self, external: Option<bool>, path: &str) -> Result<String, String> {
        if external == Some(true) {
            return std::fs::read(path)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .map_err(|e| format!("{}: openBinaryFile: {}", path, e));
        }
        if external == Some(false) {
            Err(format!("{} was not specified as input, and external files were disabled via directive.", path))
        } else {
            Err(format!("{} was not specified as input (see shellcheck -x).", path))
        }
    }

    /// Resolves `SCRIPTDIR` and the `source-path` annotations against the
    /// script's directory; returns `original` when nothing exists.
    fn find_source(&self, current: &str, external: Option<bool>, source_paths: &[String], original: &str) -> String {
        let scriptdir = match current.rfind('/') {
            Some(i) => current[..=i].to_string(),
            None => String::new(),
        };
        let adjust = |s: &str| match s.strip_prefix("SCRIPTDIR") {
            Some(rest) => format!("{}{}", scriptdir, rest.trim_start_matches('/')),
            None => s.to_string(),
        };
        let allowed = external == Some(true);
        let name = adjust(original);
        let mut candidates = vec![name.clone()];
        for sp in source_paths {
            candidates.push(PathBuf::from(adjust(sp)).join(&name).to_string_lossy().into_owned());
        }
        candidates.into_iter().find(|c| allowed && Path::new(c).is_file()).unwrap_or_else(|| original.to_string())
    }

    fn get_config(&self, filename: &str) -> Option<(String, String)> {
        let rc = rc_candidates(filename).iter().find_map(|p| read_config(p));
        let ec = get_editor_config(self, filename);
        merge_configs(rc, ec)
    }
}
