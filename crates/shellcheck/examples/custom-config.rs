// SPDX-License-Identifier: GPL-3.0-or-later
//! Build a linter with a custom configuration and check a file.
//!
//! Run with `cargo run -p shellcheck-rs --example custom-config -- path/to/script.sh`.

use shellcheck::{Linter, Severity, Shell, list_checks};

fn main() -> std::io::Result<()> {
    // Show which optional checks exist.
    for check in list_checks().iter().take(3) {
        println!("optional: {} ({:?})", check.name, check.codes);
    }

    let linter = Linter::builder()
        .shell(Shell::Dash)
        .enable_all_optional()
        // Silence two checks this codebase is happy to live with.
        .exclude([2086, 2034])
        .severity(Severity::Warning)
        .build();

    let path = std::env::args().nth(1).unwrap_or_else(|| "script.sh".to_string());
    for d in linter.check_file(&path)? {
        println!("{}:{}: SC{} {}", d.line, d.column, d.code, d.message);
    }
    Ok(())
}
