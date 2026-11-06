// SPDX-License-Identifier: GPL-3.0-or-later
// Linked only so the linkme-registered checks end up in the binary.
use shellcheck_checks as _;

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version") | Some("-V") => {
            println!("{} {}", shellcheck_lsp::SERVER_NAME, env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help") | Some("-h") => {
            println!("{} - ShellCheck language server over stdio.\n\nUsage: {} [--version | --help]\nNo arguments: speak LSP on stdin/stdout.", shellcheck_lsp::SERVER_NAME, shellcheck_lsp::SERVER_NAME);
            return;
        }
        _ => {}
    }
    if let Err(e) = shellcheck_lsp::server::run_stdio() {
        eprintln!("{}: {}", shellcheck_lsp::SERVER_NAME, e);
        std::process::exit(2);
    }
}
