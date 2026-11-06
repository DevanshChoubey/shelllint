// SPDX-License-Identifier: GPL-3.0-only
// Build script for the port of ShellCheck's checks (e11f964); no Haskell counterpart
//
// Every check lives in its own file, `src/<module>/<snake_name>.rs`. So that
// parallel porting never edits a shared file, the `mod` lines for each module
// directory are generated here: `$OUT_DIR/<module>.rs` holds one
// `#[path = "..."] pub mod <snake_name>;` per `.rs` file in `src/<module>/`,
// sorted by file name, and `src/lib.rs` `include!`s it.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The module directories under `src/`, one per Haskell module.
const MODULES: &[&str] = &["analytics", "commands", "shell_support"];

fn main() {
    let src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    for module in MODULES {
        let dir = src.join(module);
        // Re-run when a file is added, removed or renamed.
        println!("cargo:rerun-if-changed={}", dir.display());
        fs::write(out.join(format!("{module}.rs")), mod_lines(&dir)).unwrap();
    }
}

fn mod_lines(dir: &Path) -> String {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
        Err(_) => vec![],
    };
    files.retain(|p| p.extension().is_some_and(|e| e == "rs"));
    files.sort();
    let mut s = String::new();
    for p in files {
        let stem = p.file_stem().unwrap().to_str().unwrap();
        assert!(
            !stem.is_empty() && stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "{}: check file names must be snake_case",
            p.display()
        );
        s.push_str(&format!("#[path = {:?}]\npub mod {stem};\n", p.display().to_string()));
    }
    s
}
