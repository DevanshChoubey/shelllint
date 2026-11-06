// SPDX-License-Identifier: GPL-3.0-or-later
//! Language Server for shellcheck-rust, served over stdio.
//!
//! Diagnostics (debounced on change, immediate on open and save), quick fixes
//! from ShellCheck's own fixes, "disable SCxxxx for this line", hover with the
//! wiki link, `.shellcheckrc` / `.editorconfig` discovery, and `# shellcheck`
//! directives (handled by the parser). Incremental document sync.

pub mod check;
pub mod config;
pub mod document;
pub mod position;
pub mod server;
pub mod sys;
pub mod transport;

/// The binary name. Editors launch this; keep it in one place.
pub const SERVER_NAME: &str = "shellcheck-rust-lsp";
