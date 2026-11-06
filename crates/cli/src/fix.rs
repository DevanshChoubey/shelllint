// SPDX-License-Identifier: GPL-3.0-or-later
// `--fix` and `--dry-run`: an extension beyond upstream ShellCheck.
//
// One round of `--fix` is `-f diff | patch -p1`: the same check, the same
// fix merging (`fixes_by_file`) and the same application (`apply_fix`) as
// the diff formatter, written back to each file. Overlapping fixes are
// dropped by the merge, so a round can leave fixes that only apply once
// their neighbours are in; rounds repeat until nothing changes, which makes
// a second `--fix` a no-op. Then the files are linted again and whatever is
// left is printed in the chosen format.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use shellcheck_checker::check_script_owned;
use shellcheck_core::interface::{CheckSpec, Fix, SystemInterface};
use shellcheck_formatter::{Formatter, by_name, diff};

use crate::options::Options;
use crate::sys::{self, io_interface};
use crate::{Status, print_err, run_formatter};

/// Rounds of fixing before giving up on reaching a fixed point.
const MAX_ROUNDS: usize = 16;

pub(crate) fn run_fix(options: &Options, formatter: Box<dyn Formatter>, files: &[String]) -> Status {
    let mut status = Status::NoProblems;
    let mut targets: Vec<String> = vec![];
    for file in files {
        if file == "-" {
            print_err("--fix: cannot fix standard input ('-'); skipping it.");
            status = Status::RuntimeException;
        } else {
            targets.push(file.clone());
        }
    }
    if targets.is_empty() {
        return status;
    }

    if options.dry_run {
        // Exactly `-f diff` on the same files.
        let diff = by_name("diff", options.formatter_options.clone()).expect("diff formatter");
        let sys = io_interface(options, &targets);
        return status.max(run_formatter(&sys, diff, options, &targets));
    }

    let mut fixer = Fixer::new(&targets);
    let mut converged = false;
    for _ in 0..MAX_ROUNDS {
        // A fresh interface each round, so nothing read before a fix is reused.
        let sys = io_interface(options, &targets);
        if !fixer.round(&sys, options, &targets) {
            converged = true;
            break;
        }
    }
    if !converged {
        print_err(&format!("--fix: fixes still changing files after {} rounds; stopping.", MAX_ROUNDS));
    }
    if fixer.failed {
        status = Status::RuntimeException;
    }

    // Lint again and report what is left.
    let sys = io_interface(options, &targets);
    status.max(run_formatter(&sys, formatter, options, &targets))
}

struct Fixer {
    /// The command-line files, as given and canonicalized.
    given: HashSet<String>,
    given_canonical: HashSet<PathBuf>,
    /// Files already reported as not fixable or not given.
    reported: HashSet<String>,
    failed: bool,
}

impl Fixer {
    fn new(targets: &[String]) -> Self {
        Fixer {
            given: targets.iter().cloned().collect(),
            given_canonical: targets.iter().filter_map(|f| std::fs::canonicalize(f).ok()).collect(),
            reported: HashSet::new(),
            failed: false,
        }
    }

    fn is_given(&self, name: &str) -> bool {
        self.given.contains(name) || std::fs::canonicalize(name).is_ok_and(|p| self.given_canonical.contains(&p))
    }

    /// One `-f diff | patch -p1` over every file; true if any file changed.
    fn round(&mut self, sys: &dyn SystemInterface, options: &Options, targets: &[String]) -> bool {
        let mut changed = false;
        for file in targets {
            // Unreadable files are reported by the final lint.
            let Ok(contents) = sys.read_file(None, file) else { continue };
            let spec = CheckSpec { filename: file.clone(), script: contents, ..options.check_spec.clone() };
            let result = check_script_owned(sys, spec);
            for (name, fix) in diff::fixes_by_file(&result) {
                if self.reported.contains(&name) {
                    continue;
                }
                if !self.is_given(&name) {
                    print_err(&format!("--fix: not fixing {}: it was not given on the command line.", name));
                    self.reported.insert(name);
                    continue;
                }
                match fix_file(&name, &fix) {
                    Ok(c) => changed |= c,
                    Err(msg) => {
                        print_err(&format!("--fix: not fixing {}", msg));
                        self.reported.insert(name);
                        self.failed = true;
                    }
                }
            }
        }
        changed
    }
}

/// Apply `fix` to the file on disk; true if its contents changed.
fn fix_file(path: &str, fix: &Fix) -> Result<bool, String> {
    let meta = std::fs::metadata(path).map_err(|e| sys::show_io_error(path, &e))?;
    if !meta.is_file() {
        return Err(format!("{}: not a regular file.", path));
    }
    let bytes = std::fs::read(path).map_err(|e| sys::show_io_error(path, &e))?;
    let contents = std::str::from_utf8(&bytes).map_err(|_| format!("{}: it is not valid UTF-8.", path))?;
    match diff::apply_fix_to_contents(contents, fix) {
        None => Ok(false),
        Some(new) => {
            write_atomically(Path::new(path), new.as_bytes())
                .map_err(|e| format!("{}: could not write it: {}", path, e))?;
            Ok(true)
        }
    }
}

/// Write `data` to a temporary file next to `path` (after following
/// symlinks), give it the original permissions, and rename it over `path`.
fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path)?;
    let perms = std::fs::metadata(&target)?.permissions();
    let dir = target.parent().unwrap_or(Path::new("."));
    let base = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let (tmp, mut file) = create_temp(dir, &base)?;
    let result = (|| {
        file.write_all(data)?;
        file.set_permissions(perms)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn create_temp(dir: &Path, base: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    let mut last = None;
    for n in 0..100 {
        let tmp = dir.join(format!(".{}.fix-{}-{}.tmp", base, std::process::id(), n));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&tmp) {
            Ok(f) => return Ok((tmp, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no temporary file name")))
}
