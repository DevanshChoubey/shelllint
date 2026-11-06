// SPDX-License-Identifier: GPL-3.0-or-later
//! Apply every automatic fix to a script, the way `shellcheck -f diff` does.
//!
//! Run with `cargo run -p shellcheck-rs --example fix-all`.

use shellcheck::{Linter, apply_fixes};

fn main() {
    let script = "#!/bin/sh\necho $1\nrm $file\n";
    let diagnostics = Linter::new().check_str(script);

    let fixable = diagnostics.iter().filter(|d| d.fix.is_some()).count();
    println!("{} diagnostics, {} with a fix", diagnostics.len(), fixable);

    // Fixes that overlap an earlier one are dropped, as in the diff formatter.
    print!("{}", apply_fixes(script, &diagnostics));

    // A single fix can also be applied on its own.
    if let Some(fix) = diagnostics.iter().find_map(|d| d.fix.as_ref()) {
        print!("first fix only:\n{}", fix.apply(script));
    }
}
