// SPDX-License-Identifier: GPL-3.0-or-later
//! `shellcheck-rust-mcp`: serves ShellCheck over MCP on stdin/stdout.

use std::process::exit;

const USAGE: &str = "\
shellcheck-rust-mcp: MCP server (stdio transport) for the shellcheck-rust linter.
Speaks JSON-RPC 2.0, one message per line, on stdin/stdout. Logs go nowhere.

Usage: shellcheck-rust-mcp [--version | --help]
Configure it in your MCP client; see docs/mcp.md.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--version" | "-V") => {
            println!("{} {}", shellcheck_mcp::SERVER_NAME, shellcheck_mcp::SERVER_VERSION);
            return;
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return;
        }
        Some(other) => {
            eprintln!("shellcheck-rust-mcp: unexpected argument '{other}'\n{USAGE}");
            exit(2);
        }
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(e) = shellcheck_mcp::serve(stdin.lock(), stdout.lock()) {
        eprintln!("shellcheck-rust-mcp: {e}");
        exit(1);
    }
}
