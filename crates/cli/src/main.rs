// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck shellcheck.hs (e11f964)

// Linked only so the linkme-registered checks end up in the binary.
use shellcheck_checks as _;

mod options;
mod sys;

use std::io::Write;
use std::process::exit;

use shellcheck_core::data::{SHELLCHECK_VERSION, shell_for_executable};
use shellcheck_checker::check_script;
use shellcheck_core::interface::{
    CheckSpec, ColorOption, Severity, SystemInterface,
};
use shellcheck_formatter::{Formatter, by_name, tty::WIKI_LINK};

use options::{Flag, Options, default_options, get_opt, get_usage_info, options as option_table};
use sys::io_interface;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Status {
    NoProblems,
    SomeProblems,
    SupportFailure,
    SyntaxFailure,
    RuntimeException,
}

fn print_err(s: &str) {
    eprintln!("{}", s);
}

fn parse_file_list_lines(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(|l| l.trim_matches(|c: char| c.is_whitespace()))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn parse_arguments(argv: &[String]) -> Result<(Vec<Flag>, Vec<String>), Status> {
    let (opts, files, errors) = get_opt(&option_table(), argv);
    if errors.is_empty() {
        Ok((opts, files))
    } else {
        print_err(&format!("{}\n{}", errors.concat(), get_usage_info()));
        Err(Status::SyntaxFailure)
    }
}

/// `getOption`: the first flag with that name.
fn get_option<'a>(flags: &'a [Flag], name: &str) -> Option<&'a str> {
    flags.iter().find(|f| f.0 == name).map(|f| f.1.as_str())
}

fn get_options<'a>(flags: &'a [Flag], name: &str) -> Vec<&'a str> {
    flags.iter().filter(|f| f.0 == name).map(|f| f.1.as_str()).collect()
}

/// `split char str`: keeps empty fields, "" gives [""].
fn split(c: char, s: &str) -> Vec<String> {
    s.split(c).map(String::from).collect()
}

fn get_env_args() -> Vec<String> {
    let opts = std::env::var("SHELLCHECK_OPTS").unwrap_or_default();
    opts.split(' ').filter(|s| !s.is_empty()).map(String::from).collect()
}

fn main() {
    // Haskell's `error` (an uncaught ErrorCall) prints `<prog>: <message>`
    // and exits 1. A Rust panic would exit 101 with a backtrace hint, so the
    // hook prints the message the same way and `main` exits 1 once the
    // unwinding (which flushes buffered stdout) reaches it.
    // DIVERGENCE: the message text is the port's own, not GHC's (there is
    // no `CallStack (from HasCallStack)` trailer).
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "internal error".to_string());
        eprintln!("{}: {}", prog_name(), msg);
    }));
    match std::panic::catch_unwind(run_main) {
        Ok(code) => exit(code),
        Err(_) => {
            let _ = std::io::stdout().flush();
            exit(1)
        }
    }
}

fn run_main() -> i32 {
    // DIVERGENCE: non-UTF-8 arguments are converted lossily (U+FFFD).
    let params: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let mut args = get_env_args();
    args.extend(params);
    let status = match parse_arguments(&args).and_then(|(flags, files)| process(&flags, &files)) {
        Ok(s) | Err(s) => s,
    };
    status_to_code(status)
}

/// The program name, as GHC's `getProgName` gives it (the basename of argv[0]).
fn prog_name() -> String {
    std::env::args_os()
        .next()
        .map(|a| {
            let p = std::path::PathBuf::from(a);
            p.file_name().unwrap_or(p.as_os_str()).to_string_lossy().into_owned()
        })
        .unwrap_or_else(|| "shellcheck".to_string())
}

/// An uncaught Haskell exception: `<prog>: <message>` on stderr, exit 1.
fn die(msg: &str) -> ! {
    let _ = std::io::stdout().flush();
    eprintln!("{}: {}", prog_name(), msg);
    exit(1)
}

fn status_to_code(status: Status) -> i32 {
    match status {
        Status::NoProblems => 0,
        Status::SomeProblems => 1,
        Status::SyntaxFailure => 3,
        Status::SupportFailure => 4,
        Status::RuntimeException => 2,
    }
}

fn process(flags: &[Flag], files: &[String]) -> Result<Status, Status> {
    let mut options = default_options();
    for flag in flags {
        parse_option(flag, &mut options)?;
    }

    let files_from = get_options(flags, "files-from");
    let mut all_files = vec![];
    for path in &files_from {
        all_files.extend(read_files_from(path)?);
    }
    all_files.extend(files.iter().cloned());

    // It shouldn't be an error to do --files-from=/dev/null
    if files_from.is_empty() {
        verify_files(&all_files)?;
    }

    let format = get_option(flags, "format").unwrap_or("tty");
    let formatter = match by_name(format, options.formatter_options.clone()) {
        Some(f) => f,
        None => {
            print_err(&format!("Unknown format {}", format));
            print_err("Supported formats:");
            for name in options::FORMAT_NAMES {
                print_err(&format!("  {}", name));
            }
            return Err(Status::SupportFailure);
        }
    };

    let sys = io_interface(&options, &all_files);
    Ok(run_formatter(&sys, formatter, &options, &all_files))
}

fn read_files_from(path: &str) -> Result<Vec<String>, Status> {
    if path == "-" {
        // DIVERGENCE: stdin is read as UTF-8 here (invalid input yields an empty
        // list); Haskell's getContents uses the locale encoding.
        return match sys::read_stdin(true) {
            Ok(bytes) => Ok(parse_file_list_lines(&String::from_utf8(bytes).unwrap_or_default())),
            // A second `getContents` throws an IOException nothing catches.
            Err(e) => die(&e.to_string()),
        };
    }
    match sys::input_file(path) {
        Err(e) => {
            print_err(&format!(
                "Could not read file list: {}: {}",
                path,
                sys::show_io_error_in("openFile", path, &e)
            ));
            Err(Status::RuntimeException)
        }
        Ok((contents, _)) => Ok(parse_file_list_lines(&contents)),
    }
}

fn run_formatter(
    sys: &dyn SystemInterface,
    mut format: Box<dyn Formatter>,
    options: &Options,
    files: &[String],
) -> Status {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    format.header(&mut out);
    let mut result = Status::NoProblems;
    for file in files {
        let new_status = process_file(sys, format.as_mut(), options, file, &mut out);
        result = result.max(new_status);
        let _ = out.flush();
        // DIVERGENCE: Haskell's Quiet formatter calls exitFailure itself.
        if format.exit_requested() {
            exit(1);
        }
    }
    format.footer(&mut out);
    let _ = out.flush();
    result
}

fn process_file(
    sys: &dyn SystemInterface,
    format: &mut dyn Formatter,
    options: &Options,
    filename: &str,
    out: &mut dyn Write,
) -> Status {
    let contents = match sys.read_file(None, filename) {
        Ok(c) => c,
        Err(msg) => {
            format.on_failure(filename, &msg, out);
            return Status::RuntimeException;
        }
    };
    let spec = CheckSpec {
        filename: filename.to_string(),
        script: contents,
        ..options.check_spec.clone()
    };
    let result = check_script(sys, &spec);
    format.on_result(&result, sys, out);
    // A malformed EditorConfig (invalid 'root' or 'shellcheck.*' directive)
    // means shellcheck cannot apply the requested configuration.
    if result.comments.iter().any(|pc| {
        pc.comment.code == 1134 && pc.start_pos.file.ends_with(".editorconfig")
    }) {
        Status::SupportFailure
    } else if result.comments.is_empty() {
        Status::NoProblems
    } else {
        Status::SomeProblems
    }
}

fn parse_enum<T: Copy>(name: &str, value: &str, list: &[(&str, T)]) -> Result<T, Status> {
    match list.iter().find(|(k, _)| *k == value) {
        Some((_, v)) => Ok(*v),
        None => {
            let names: Vec<&str> = list.iter().map(|(k, _)| *k).collect();
            print_err(&format!(
                "Unknown value for --{}. Valid options are: {}",
                name,
                names.join(", ")
            ));
            Err(Status::SupportFailure)
        }
    }
}

fn parse_num(s: &str) -> Result<i64, Status> {
    let mut num = s;
    while let Some(rest) = num.strip_prefix("SC") {
        num = rest;
    }
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        print_err(&format!("Invalid number: {}", num));
        return Err(Status::SyntaxFailure);
    }
    // DIVERGENCE: Haskell's `read ""` crashes on an empty number (rejected above
    // instead); Integer is unbounded, here values past i64::MAX saturate.
    Ok(num.parse::<i64>().unwrap_or(i64::MAX))
}

fn parse_bool(s: &str) -> Result<bool, Status> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            print_err(&format!("Invalid boolean, expected true/false: {}", s));
            Err(Status::SyntaxFailure)
        }
    }
}

fn parse_option(flag: &Flag, options: &mut Options) -> Result<(), Status> {
    let Flag(name, val) = flag;
    match name.as_str() {
        "shell" => match shell_for_executable(val) {
            Some(shell) => options.check_spec.shell_type_override = Some(shell),
            None => {
                print_err(&format!("Unknown shell: {}", val));
                return Err(Status::SupportFailure);
            }
        },
        "exclude" => {
            let mut new = vec![];
            for s in split(',', val).iter().filter(|s| !s.is_empty()) {
                new.push(parse_num(s)?);
            }
            new.extend(options.check_spec.excluded_warnings.drain(..));
            options.check_spec.excluded_warnings = new;
        }
        "include" => {
            let mut new = vec![];
            for s in split(',', val).iter().filter(|s| !s.is_empty()) {
                new.push(parse_num(s)?);
            }
            if !new.is_empty() {
                // Just new `mappend` old
                if let Some(old) = options.check_spec.included_warnings.take() {
                    new.extend(old);
                }
                options.check_spec.included_warnings = Some(new);
            }
        }
        "version" => {
            print_version();
            return Err(Status::NoProblems);
        }
        "list-optional" => {
            print_optional();
            return Err(Status::NoProblems);
        }
        "help" => {
            println!("{}", get_usage_info());
            return Err(Status::NoProblems);
        }
        "externals" => options.external_sources = true,
        "color" => {
            options.formatter_options.color_option = parse_enum(
                "color",
                val,
                &[
                    ("auto", ColorOption::ColorAuto),
                    ("always", ColorOption::ColorAlways),
                    ("never", ColorOption::ColorNever),
                ],
            )?;
        }
        "source-path" => options.source_paths.extend(split_search_path(val)),
        "sourced" => options.check_spec.check_sourced = true,
        "severity" => {
            options.check_spec.min_severity = parse_enum(
                "severity",
                val,
                &[
                    ("error", Severity::ErrorC),
                    ("warning", Severity::WarningC),
                    ("info", Severity::InfoC),
                    ("style", Severity::StyleC),
                ],
            )?;
        }
        "wiki-link-count" => options.formatter_options.wiki_link_count = parse_num(val)?,
        "norc" => options.check_spec.ignore_rc = true,
        "rcfile" => options.rcfile = Some(val.clone()),
        "enable" => options.check_spec.optional_checks.extend(split(',', val)),
        "extended-analysis" => options.check_spec.extended_analysis = Some(parse_bool(val)?),
        // These flags are handled specially in 'process'
        "format" | "files-from" => {}
        other => print_err(&format!("Internal error for --{}. Please file a bug :(", other)),
    }
    Ok(())
}

/// System.FilePath `splitSearchPath` (POSIX): empty components become ".".
fn split_search_path(s: &str) -> Vec<String> {
    s.split(':')
        .map(|p| if p.is_empty() { ".".to_string() } else { p.to_string() })
        .collect()
}

fn verify_files(files: &[String]) -> Result<(), Status> {
    if files.is_empty() {
        print_err("No files specified.\n");
        print_err(&get_usage_info());
        return Err(Status::SyntaxFailure);
    }
    Ok(())
}

fn print_version() {
    println!("shellcheck-rust - shell script analysis tool (a Rust port of ShellCheck)");
    println!(
        "version: {} (ports ShellCheck v{}-171-ge11f964)",
        env!("CARGO_PKG_VERSION"),
        SHELLCHECK_VERSION
    );
    println!("license: GNU General Public License, version 3");
    println!("website: https://www.shellcheck.net");
}

fn print_optional() {
    let mut list = shellcheck_analyzer::api::analytics_optional_checks();
    list.extend(shellcheck_analyzer::api::commands_optional_checks());
    // DIVERGENCE: ControlFlow's optional checks arrive with Phase 3.
    list.sort_by(|a, b| a.name.cmp(&b.name));
    for item in list {
        println!("name:    {}", item.name);
        let ids: Vec<String> = item.id.iter().map(|n| format!("SC{}", n)).collect();
        println!("id:      {}", ids.join(", "));
        let wikis: Vec<String> = item.id.iter().map(|n| format!("{}SC{}", WIKI_LINK, n)).collect();
        println!("wiki:    {}", wikis.join(", "));
        println!("desc:    {}", item.description);
        println!("example: {}", item.positive);
        println!("fix:     {}", item.negative);
        println!();
    }
}
