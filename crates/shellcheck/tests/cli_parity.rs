// SPDX-License-Identifier: GPL-3.0-or-later
//! The facade's diagnostics match the CLI's `-f json1` output.
#![cfg(feature = "serde")]

mod common;

use common::{fixture, require_cli, run_cli};
use shellcheck::{Linter, Shell};

/// (fixture, shell name for the CLI, shell for the facade)
const CASES: &[(&str, &str, Shell)] = &[
    ("fix_basic.sh", "sh", Shell::Sh),
    ("no_trailing_newline.sh", "bash", Shell::Bash),
    ("tabs.sh", "bash", Shell::Bash),
    ("backticks.sh", "bash", Shell::Bash),
    ("function.sh", "sh", Shell::Sh),
    ("latin1.sh", "sh", Shell::Sh),
];

#[test]
fn facade_matches_cli_json1() {
    let Some(bin) = require_cli("facade_matches_cli_json1") else { return };
    for &(name, cli_shell, shell) in CASES {
        let path = fixture(name);
        let cli = run_cli(&bin, &["--norc", "-o", "all", "-s", cli_shell, "-f", "json1"], &path);
        let cli: serde_json::Value = serde_json::from_str(&cli).expect("CLI json1 parses");
        let mut expected = cli["comments"].as_array().expect("comments array").clone();
        for c in &mut expected {
            c.as_object_mut().unwrap().remove("file");
        }

        let linter = Linter::builder().shell(shell).enable_all_optional().build();
        let got = serde_json::to_value(linter.check_file(&path).expect("read fixture")).unwrap();

        assert_eq!(got, serde_json::Value::Array(expected), "mismatch for {name}");
    }
}
