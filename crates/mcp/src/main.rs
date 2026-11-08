// SPDX-License-Identifier: GPL-3.0-or-later
//! `shelllint-mcp`: serves ShellCheck over MCP on stdin/stdout.

use std::process::exit;

const USAGE: &str = "\
shelllint-mcp: MCP server (stdio transport) for the shelllint linter.
Speaks JSON-RPC 2.0, one message per line, on stdin/stdout. Logs go nowhere.

Usage: shelllint-mcp [--version | --help]
Configure it in your MCP client; see docs/mcp.md.";

fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--version" | "-V") => {
            println!("{} {}", shelllint_mcp::SERVER_NAME, shelllint_mcp::SERVER_VERSION);
            return;
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return;
        }
        Some(other) => {
            eprintln!("shelllint-mcp: unexpected argument '{other}'\n{USAGE}");
            exit(2);
        }
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(e) = shelllint_mcp::serve(stdin.lock(), stdout.lock()) {
        eprintln!("shelllint-mcp: {e}");
        exit(1);
    }
}
