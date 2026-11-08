// SPDX-License-Identifier: GPL-3.0-or-later
fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version") | Some("-V") => {
            println!("{} {}", shelllint_lsp::SERVER_NAME, env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help") | Some("-h") => {
            println!(
                "{} - ShellCheck language server over stdio.\n\nUsage: {} [--version | --help]\nNo arguments: speak LSP on stdin/stdout.",
                shelllint_lsp::SERVER_NAME,
                shelllint_lsp::SERVER_NAME
            );
            return;
        }
        _ => {}
    }
    if let Err(e) = shelllint_lsp::server::run_stdio() {
        eprintln!("{}: {}", shelllint_lsp::SERVER_NAME, e);
        std::process::exit(2);
    }
}
