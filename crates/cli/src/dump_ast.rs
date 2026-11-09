// SPDX-License-Identifier: GPL-3.0-or-later
// `--dump-ast`: an extension beyond upstream ShellCheck (issue #2696).
// One JSON document per file on stdout (a bare document for one file, an
// array for several), instead of the normal report. The document shape is
// described in `shelllint_checker::ast_dump`.

use std::io::Write;

use shelllint_checker::ast_dump::{dump_ast, dump_read_failure};
use shelllint_core::interface::{CheckSpec, SystemInterface};

use crate::Status;
use crate::options::Options;
use crate::sys::io_interface;

/// Exit status: 0 if every file parsed, 1 (`SomeProblems`) if a file did not
/// parse, 2 if a file could not be read.
pub(crate) fn run(options: &Options, files: &[String]) -> Status {
    let sys = io_interface(options, files);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let many = files.len() != 1;
    let mut status = Status::NoProblems;
    if many {
        let _ = out.write_all(b"[");
    }
    for (i, file) in files.iter().enumerate() {
        if i > 0 {
            let _ = out.write_all(b",\n");
        }
        let s = dump_file(&sys, options, file, &mut out);
        status = status.max(s);
    }
    if many {
        let _ = out.write_all(b"]");
    }
    let _ = out.write_all(b"\n");
    let _ = out.flush();
    status
}

fn dump_file(sys: &dyn SystemInterface, options: &Options, filename: &str, out: &mut dyn Write) -> Status {
    match sys.read_file(None, filename) {
        Err(msg) => {
            crate::print_err(&format!("{}: {}", filename, msg));
            let _ = dump_read_failure(filename, &msg, out);
            Status::RuntimeException
        }
        Ok(contents) => {
            let spec = CheckSpec { filename: filename.to_string(), script: contents, ..options.check_spec.clone() };
            match dump_ast(sys, spec, out) {
                Ok(true) => Status::NoProblems,
                // A broken pipe on stdout is not worth a different status.
                Ok(false) | Err(_) => Status::SomeProblems,
            }
        }
    }
}
