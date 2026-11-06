// SPDX-License-Identifier: GPL-3.0-or-later
//! Lint a script from a string and print each diagnostic.
//!
//! Run with `cargo run -p shellcheck-rs --example basic`.

use shellcheck::{Linter, Shell};

fn main() {
    let script = "#!/bin/bash\nfiles=$(ls)\nfor f in $files; do\n  echo $f\ndone\n";

    let linter = Linter::builder().shell(Shell::Bash).build();
    for d in linter.check_str(script) {
        println!(
            "{}:{}-{}:{} {:?} SC{}: {}",
            d.line, d.column, d.end_line, d.end_column, d.level, d.code, d.message
        );
    }
}
