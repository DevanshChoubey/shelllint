// SPDX-License-Identifier: GPL-3.0-or-later
//! The `SystemInterface` the server checks with. Config discovery mirrors the
//! CLI (`crates/cli/src/sys.rs`): `.shellcheckrc` / `shellcheckrc` walking up
//! from the script, then the home and XDG config files, merged with
//! `.editorconfig` through `shelllint_checker::editorconfig`.
//! Files are read from disk on every check, so rc edits apply on the next run.

use std::path::{Path, PathBuf};

use lsp_types::Uri;
use shelllint_checker::decode_string;
use shelllint_checker::editorconfig::{EditorConfigFs, get_editor_config, merge_configs};
use shelllint_checker::filepath::{combine, drop_file_name, is_path_separator, strip_verbatim};
use shelllint_core::interface::SystemInterface;

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

/// `$HOME`, or on Windows the AppData folder the Haskell directory package uses.
fn user_dir() -> Option<String> {
    std::env::var(if cfg!(windows) { "APPDATA" } else { "HOME" }).ok().filter(|h| !h.is_empty())
}

fn xdg_dir() -> Option<String> {
    let fallback = user_dir().map(|h| if cfg!(windows) { h } else { combine(&h, ".config") });
    std::env::var("XDG_CONFIG_HOME").ok().filter(|h| !h.is_empty()).or(fallback)
}

/// Candidate rc paths for `filename`: nearest directory first, then the
/// user-level files.
fn rc_candidates(filename: &str) -> Vec<String> {
    let mut out = vec![];
    let mut dir = dir_of(filename);
    loop {
        out.push(combine(&dir, ".shellcheckrc"));
        out.push(combine(&dir, "shellcheckrc"));
        let next = dir_of(&dir);
        if next == dir {
            break;
        }
        dir = next;
    }
    if let Some(home) = user_dir() {
        out.push(combine(&home, if cfg!(windows) { "shellcheckrc" } else { ".shellcheckrc" }));
    }
    if let Some(x) = xdg_dir() {
        out.push(combine(&x, "shellcheckrc"));
    }
    out
}

/// Haskell `show` of the IOException `openBinaryFile` raises, as the CLI
/// prints it in SC1091 ("does not exist (No such file or directory)").
///
/// Copied from `show_io_error_in` in crates/cli/src/sys.rs (the CLI's table is
/// the reference and is being extended to more errnos; keep the two in step).
pub fn show_io_error(file: &str, e: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    let desc = match e.kind() {
        NotFound => "does not exist (No such file or directory)".to_string(),
        PermissionDenied => "permission denied (Permission denied)".to_string(),
        _ if e.raw_os_error() == Some(21) => "inappropriate type (is a directory)".to_string(),
        _ => format!("failed ({})", e),
    };
    format!("{}: openBinaryFile: {}", file, desc)
}

impl EditorConfigFs for LspSystem {
    fn normalize(&self, path: &str) -> String {
        std::fs::canonicalize(path).map(|p| strip_verbatim(&p.to_string_lossy())).unwrap_or_else(|_| path.to_string())
    }

    fn read_config(&self, file: &str) -> Option<(String, String)> {
        read_config(file)
    }

    fn global_editor_config_path(&self) -> Option<String> {
        Some(combine(&xdg_dir()?, "editorconfig.ini"))
    }
}

impl SystemInterface for LspSystem {
    /// Only the file being edited is an input; sourcing other files needs the
    /// rc to say `external-sources`, as on the CLI.
    fn read_file(&self, external: Option<bool>, path: &str) -> Result<String, String> {
        if external == Some(true) {
            return std::fs::read(path).map(|b| decode_string(&b)).map_err(|e| show_io_error(path, &e));
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
        let scriptdir = drop_file_name(current);
        let adjust = |s: &str| match s.strip_prefix("SCRIPTDIR") {
            Some(rest) => format!("{}{}", scriptdir, rest.trim_start_matches(is_path_separator)),
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

/// The filesystem path a `file:` URI names, or `None` when it names none here.
/// Same rules as `url::Url::to_file_path`: Unix rejects a remote host, Windows
/// needs a drive or a host (`\\\\host\\share`) and accepts `c%3A`, the form
/// VS Code sends. `windows` is a parameter so every platform can test both.
pub fn file_uri_path(uri: &Uri, windows: bool) -> Option<String> {
    let host = uri.authority().map(|a| a.as_str()).filter(|h| !h.is_empty() && !h.eq_ignore_ascii_case("localhost"));
    if !windows {
        return match host {
            Some(_) => None,
            None => Some(uri.path().as_estr().decode().into_string_lossy().into_owned()),
        };
    }
    let mut segments = uri.path().segments();
    let mut out = match host {
        Some(h) => format!(r"\\{h}"),
        None => {
            let drive = segments.next()?.decode().into_string().ok()?;
            let is_drive = matches!(drive.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic());
            if !is_drive {
                return None;
            }
            drive.into_owned()
        }
    };
    for segment in segments {
        out.push('\\');
        out.push_str(&segment.decode().into_string().ok()?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_of(uri: &str, windows: bool) -> Option<String> {
        file_uri_path(&uri.parse().unwrap(), windows)
    }

    #[test]
    fn file_uris_decode_to_unix_paths() {
        assert_eq!(path_of("file:///tmp/a%20b.sh", false).as_deref(), Some("/tmp/a b.sh"));
        assert_eq!(path_of("file://localhost/tmp/x.sh", false).as_deref(), Some("/tmp/x.sh"));
        assert_eq!(path_of("file:///caf%C3%A9.sh", false).as_deref(), Some("/caf\u{e9}.sh"));
        assert_eq!(path_of("file://server/tmp/x.sh", false), None);
    }

    #[test]
    fn file_uris_decode_to_windows_paths() {
        assert_eq!(path_of("file:///C:/a/b%20c.sh", true).as_deref(), Some(r"C:\a\b c.sh"));
        assert_eq!(path_of("file:///c%3A/x.sh", true).as_deref(), Some(r"c:\x.sh"));
        assert_eq!(path_of("file:///C%3a/", true).as_deref(), Some("C:\\"));
        assert_eq!(path_of("file://server/share/x.sh", true).as_deref(), Some(r"\\server\share\x.sh"));
        assert_eq!(path_of("file://localhost/C:/x.sh", true).as_deref(), Some(r"C:\x.sh"));
        assert_eq!(path_of("file:///nodrive/x.sh", true), None);
        assert_eq!(path_of("file:///1:/x.sh", true), None);
        assert_eq!(path_of("file:///", true), None);
    }

    #[test]
    fn read_errors_use_the_cli_wording() {
        assert_eq!(
            LspSystem.read_file(Some(true), "/nonexistent/lib.sh"),
            Err("/nonexistent/lib.sh: openBinaryFile: does not exist (No such file or directory)".to_string())
        );
        // Opening a directory fails with EISDIR only on Unix; Windows reports it differently.
        if !cfg!(unix) {
            return;
        }
        let dir = std::env::temp_dir();
        let d = dir.to_string_lossy();
        assert_eq!(
            LspSystem.read_file(Some(true), &d),
            Err(format!("{d}: openBinaryFile: inappropriate type (is a directory)"))
        );
    }
}
