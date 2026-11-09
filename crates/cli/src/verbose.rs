// SPDX-License-Identifier: GPL-3.0-or-later
// `--verbose` (extension beyond ShellCheck): report on stderr which file is
// being checked and which .shellcheckrc applies to it. Stdout is untouched.

use crate::options::Options;
use crate::sys::IoInterface;

pub fn announce(sys: &IoInterface, options: &Options, filename: &str) {
    if !options.verbose {
        return;
    }
    let rc = if options.check_spec.ignore_rc { None } else { sys.rc_path(filename) };
    eprintln!("shelllint: checking {}", filename);
    eprintln!("shelllint: using rcfile {}", rc.as_deref().unwrap_or("none"));
}
