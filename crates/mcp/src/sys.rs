// SPDX-License-Identifier: GPL-3.0-or-later
//! The `SystemInterface` the tools lint with: the CLI's semantics
//! (`crates/cli/src/sys.rs`, `shellcheck --norc` without `-x`). Only the file
//! being checked is an input, so a `source`d file is reported as SC1091 with
//! ShellCheck's own "was not specified as input" text. No rc file is read.

use std::path::{Path, PathBuf};

use shelllint_checker::decode_string;
use shelllint_checker::filepath::{drop_drive, drop_file_name, is_path_separator, strip_verbatim};
use shelllint_core::interface::{ErrorMessage, SystemInterface};

pub struct McpSystem {
    /// Canonical paths of the inputs (the `check_file` path; none for `check_script`).
    inputs: Vec<String>,
}

impl McpSystem {
    pub fn new(inputs: &[&str]) -> McpSystem {
        McpSystem { inputs: inputs.iter().map(|f| normalize(f)).collect() }
    }

    fn allowable(&self, external: Option<bool>, x: &str) -> bool {
        // The CLI's `external_sources` option is off: only the rc could turn it on.
        external.unwrap_or(false) || self.inputs.contains(&normalize(x))
    }
}

fn normalize(x: &str) -> String {
    std::fs::canonicalize(x).map(|p| strip_verbatim(&p.to_string_lossy())).unwrap_or_else(|_| x.to_string())
}

/// Haskell `show` of the IOException `openBinaryFile` raises.
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

impl SystemInterface for McpSystem {
    fn read_file(&self, external: Option<bool>, file: &str) -> Result<String, ErrorMessage> {
        if self.allowable(external, file) {
            std::fs::read(file).map(|b| decode_string(&b)).map_err(|e| show_io_error(file, &e))
        } else if external == Some(false) {
            Err(format!("{} was not specified as input, and external files were disabled via directive.", file))
        } else {
            Err(format!("{} was not specified as input (see shellcheck -x).", file))
        }
    }

    /// The CLI's `findSource`, with no `--source-path` options.
    fn find_source(&self, current: &str, external: Option<bool>, source_paths: &[String], original: &str) -> String {
        let scriptdir = drop_file_name(current);
        let adjust = |s: &str| -> String {
            let mut parts = s.split(is_path_separator).filter(|p| !p.is_empty());
            if s.starts_with("SCRIPTDIR") && parts.next() == Some("SCRIPTDIR") {
                let mut p = PathBuf::from(scriptdir);
                for r in parts {
                    p.push(r);
                }
                p.to_string_lossy().into_owned()
            } else {
                s.to_string()
            }
        };
        let find = |filename: &str, deflt: &str| -> String {
            let mut candidates = vec![adjust(filename)];
            for sp in source_paths {
                candidates.push(Path::new(&adjust(sp)).join(filename).to_string_lossy().into_owned());
            }
            candidates
                .into_iter()
                .find(|c| self.allowable(external, c) && std::fs::metadata(c).is_ok_and(|m| !m.is_dir()))
                .unwrap_or_else(|| deflt.to_string())
        };
        if Path::new(original).is_absolute() { find(drop_drive(original), original) } else { find(original, original) }
    }

    fn get_config(&self, _filename: &str) -> Option<(String, String)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_inputs_get_the_cli_message() {
        let sys = McpSystem::new(&[]);
        assert_eq!(
            sys.read_file(None, "./lib.sh"),
            Err("./lib.sh was not specified as input (see shellcheck -x).".into())
        );
        assert_eq!(
            sys.read_file(Some(false), "./lib.sh"),
            Err("./lib.sh was not specified as input, and external files were disabled via directive.".into())
        );
        assert_eq!(
            sys.read_file(Some(true), "/nonexistent/q.sh"),
            Err("/nonexistent/q.sh: openBinaryFile: does not exist (No such file or directory)".into())
        );
    }
}
