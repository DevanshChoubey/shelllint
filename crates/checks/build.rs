// SPDX-License-Identifier: GPL-3.0-or-later
// Build script for the port of ShellCheck's checks (e11f964); no Haskell counterpart
//
// Every check lives in its own file, `src/<module>/<snake_name>.rs`. So that
// parallel porting never edits a shared file, two things are generated here:
//
// - `$OUT_DIR/<module>.rs`: one `#[path = "..."] pub mod <snake_name>;` per
//   `.rs` file in `src/<module>/`, sorted by file name; `src/lib.rs`
//   `include!`s it into `pub mod <module>`.
// - `$OUT_DIR/registry.rs`: `pub static CHECKS`, a reference to every
//   registered `CheckEntry`. A file registers an entry by declaring, at the
//   start of a line, `pub(crate) static <NAME>: CheckEntry = ...`, or by
//   invoking `parametrised_command_checks!` (which declares
//   `pub(crate) static ENTRIES: [CheckEntry; N]`, one per listed command).
//   Order: the modules in `MODULES` order, the files sorted by name, the
//   entries in source order. Only ties in a registry's `order` can observe
//   it (`api::registered` sorts stably by `order`); the only ties are the
//   entries of one `parametrised_command_checks!`, which keep list order.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The module directories under `src/`, one per Haskell module.
const MODULES: &[&str] = &["analytics", "commands", "shell_support"];

const STATIC_PREFIX: &str = "pub(crate) static ";
const MACRO: &str = "parametrised_command_checks!(";

fn main() {
    let src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut registry = String::from(
        "/// Every registered check (see `build.rs` for how files register and\n\
         /// the order). Pass it to the `shellcheck_analyzer::api` and\n\
         /// `shellcheck_analyzer::testing` functions that take a registry.\n\
         pub static CHECKS: &[&::shellcheck_analyzer::api::CheckEntry] = &[\n",
    );
    let mut asserts = String::new();
    for module in MODULES {
        let dir = src.join(module);
        // Re-run when a file is added, removed or renamed.
        println!("cargo:rerun-if-changed={}", dir.display());
        let files = check_files(&dir);
        let mut mods = String::new();
        for p in &files {
            // Re-run when a registration is added or removed.
            println!("cargo:rerun-if-changed={}", p.display());
            let stem = p.file_stem().unwrap().to_str().unwrap();
            mods.push_str(&format!("#[path = {:?}]\npub mod {stem};\n", p.display().to_string()));
            let text = fs::read_to_string(p).unwrap();
            for r in registrations(p, &text) {
                match r {
                    Reg::Static(name) => registry.push_str(&format!("    &crate::{module}::{stem}::{name},\n")),
                    Reg::Parametrised(n) => {
                        for i in 0..n {
                            registry.push_str(&format!("    &crate::{module}::{stem}::ENTRIES[{i}],\n"));
                        }
                        asserts.push_str(&format!(
                            "const _: () = assert!(crate::{module}::{stem}::ENTRIES.len() == {n});\n"
                        ));
                    }
                }
            }
        }
        fs::write(out.join(format!("{module}.rs")), mods).unwrap();
    }
    registry.push_str("];\n");
    registry.push_str(&asserts);
    fs::write(out.join("registry.rs"), registry).unwrap();
}

fn check_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
        Err(_) => vec![],
    };
    files.retain(|p| p.extension().is_some_and(|e| e == "rs"));
    files.sort();
    for p in &files {
        let stem = p.file_stem().unwrap().to_str().unwrap();
        assert!(
            !stem.is_empty() && stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "{}: check file names must be snake_case",
            p.display()
        );
    }
    files
}

enum Reg {
    /// `pub(crate) static NAME: CheckEntry`
    Static(String),
    /// `parametrised_command_checks!` with this many commands.
    Parametrised(usize),
}

/// The registrations in one file, in source order.
fn registrations(path: &Path, text: &str) -> Vec<Reg> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix(STATIC_PREFIX) {
            let Some((name, ty)) = rest.split_once(':') else { continue };
            if !ty.trim_start().starts_with("CheckEntry ") {
                continue;
            }
            assert!(
                !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                "{}: bad registered static name {name:?}",
                path.display()
            );
            out.push(Reg::Static(name.to_string()));
        } else if line.starts_with(MACRO) {
            // Collect the invocation up to its closing `);`.
            let mut call = line.to_string();
            while !call.trim_end().ends_with(");") {
                let next = lines.next().unwrap_or_else(|| panic!("{}: unterminated {MACRO}", path.display()));
                call.push('\n');
                call.push_str(next);
            }
            let open = call.rfind('[').unwrap_or_else(|| panic!("{}: {MACRO} without a list", path.display()));
            let close = call.rfind(']').unwrap();
            let quotes = call[open..close].matches('"').count();
            assert!(quotes > 0 && quotes % 2 == 0, "{}: cannot count the {MACRO} list", path.display());
            out.push(Reg::Parametrised(quotes / 2));
        }
    }
    assert!(
        out.iter().filter(|r| matches!(r, Reg::Parametrised(_))).count() <= 1,
        "{}: at most one {MACRO} per file",
        path.display()
    );
    out
}
