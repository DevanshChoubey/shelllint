// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck shellcheck.hs (e11f964): `Flag`, `options`, `usageInfo`
// and System.Console.GetOpt's `getOpt Permute`.

use shelllint_core::interface::{
    CheckSpec, ColorOption, FormatterOptions, Severity, empty_check_spec, new_formatter_options,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flag(pub String, pub String);

pub struct Options {
    pub check_spec: CheckSpec,
    pub external_sources: bool,
    pub source_paths: Vec<String>,
    pub formatter_options: FormatterOptions,
    #[allow(dead_code)]
    pub min_severity: Severity,
    pub rcfile: Option<String>,
    /// `--fix` (an extension beyond ShellCheck): apply the fixes in place.
    pub fix: bool,
    /// `--dry-run` (extension): with `--fix`, print the diff instead.
    pub dry_run: bool,
    /// `--verbose` (extension): name each file and its .shellcheckrc on stderr.
    pub verbose: bool,
    /// `--fallback-shell` and friends (extension).
    pub shell_inference: crate::shell_inference::ShellInference,
    /// `--dump-ast` (extension): print each file's parsed AST as JSON.
    pub dump_ast: bool,
    /// `--summary` (extension): print finding counts and files checked on stderr.
    pub summary: bool,
    /// `--fail-fast` (extension): stop after the first file with findings.
    pub fail_fast: bool,
    /// `--stdin-filename=PATH` (extension): the name `-` is reported as.
    pub stdin_filename: Option<String>,
    /// `--output=FORMAT:PATH` (extension), repeatable: (format, path).
    pub outputs: Vec<(String, String)>,
    /// `--cache-dir=PATH` (extension): where to keep reusable results.
    pub cache_dir: Option<String>,
}

pub fn default_options() -> Options {
    Options {
        check_spec: empty_check_spec(),
        external_sources: false,
        source_paths: vec![],
        formatter_options: FormatterOptions { color_option: ColorOption::ColorAuto, ..new_formatter_options() },
        min_severity: Severity::StyleC,
        rcfile: None,
        fix: false,
        dry_run: false,
        verbose: false,
        shell_inference: Default::default(),
        dump_ast: false,
        summary: false,
        fail_fast: false,
        stdin_filename: None,
        outputs: vec![],
        cache_dir: None,
    }
}

pub const USAGE_HEADER: &str = "Usage: shellcheck [OPTIONS...] FILES...";

/// The formats shellcheck.hs registers (`Map.keys`, so sorted).
pub const FORMAT_NAMES: [&str; 8] = ["checkstyle", "diff", "gcc", "json", "json1", "quiet", "sarif", "tty"];

#[derive(Clone, Copy)]
pub enum ArgDescr {
    /// `NoArg $ Flag name value`
    No(&'static str),
    /// `ReqArg (Flag name) placeholder`
    Req(&'static str),
    /// `OptArg (maybe (Flag name default) (Flag name)) placeholder`
    Opt(&'static str, &'static str),
}

pub struct OptDescr {
    pub shorts: &'static str,
    pub longs: &'static [&'static str],
    pub arg: ArgDescr,
    pub placeholder: &'static str,
    pub desc: &'static str,
}

use ArgDescr::*;

/// `options` (descriptions that need `formatList` are built in `options()`).
const fn o(
    shorts: &'static str,
    longs: &'static [&'static str],
    arg: ArgDescr,
    placeholder: &'static str,
    desc: &'static str,
) -> OptDescr {
    OptDescr { shorts, longs, arg, placeholder, desc }
}

pub fn options() -> Vec<OptDescr> {
    vec![
        o("a", &["check-sourced"], No("sourced"), "", "Include warnings from sourced files"),
        o("C", &["color"], Opt("color", "always"), "WHEN", "Use color (auto, always, never)"),
        o("i", &["include"], Req("include"), "CODE1,CODE2..", "Consider only given types of warnings"),
        o("e", &["exclude"], Req("exclude"), "CODE1,CODE2..", "Exclude types of warnings"),
        o("", &["extended-analysis"], Req("extended-analysis"), "bool", "Perform dataflow analysis (default true)"),
        o("f", &["format"], Req("format"), "FORMAT", FORMAT_DESC),
        o("", &["list-optional"], No("list-optional"), "", "List checks disabled by default"),
        o("", &["norc"], No("norc"), "", "Don't look for .shellcheckrc and .editorconfig files"),
        o("", &["rcfile"], Req("rcfile"), "RCFILE", "Prefer the specified configuration file over searching for one"),
        o("o", &["enable"], Req("enable"), "check1,check2..", "List of optional checks to enable (or 'all')"),
        o(
            "P",
            &["source-path"],
            Req("source-path"),
            "SOURCEPATHS",
            "Specify path when looking for sourced files (\"SCRIPTDIR\" for script's dir)",
        ),
        o("s", &["shell"], Req("shell"), "SHELLNAME", "Specify dialect (sh, bash, dash, ksh, busybox)"),
        o(
            "S",
            &["severity"],
            Req("severity"),
            "SEVERITY",
            "Minimum severity of errors to consider (error, warning, info, style)",
        ),
        o("V", &["version"], No("version"), "", "Print version information"),
        o(
            "W",
            &["wiki-link-count"],
            Req("wiki-link-count"),
            "NUM",
            "The number of wiki links to show, when applicable",
        ),
        o("x", &["external-sources"], No("externals"), "", "Allow 'source' outside of FILES"),
        o("", &["help"], No("help"), "", "Show this usage summary and exit"),
        o(
            "",
            &["files-from"],
            Req("files-from"),
            "FILE",
            "Read input files from FILE (one per line, or '-' for stdin)",
        ),
        // Extensions beyond ShellCheck (not in upstream's --help).
        o("", &["fix"], No("fix"), "", "Apply all auto-fixes to FILES in place, then report what is left"),
        o("", &["dry-run"], No("dry-run"), "", "With --fix, print the fixes as a diff (like -f diff) instead"),
        o("", &["verbose"], No("verbose"), "", "Print the file and the .shellcheckrc used for it to stderr"),
        o("", &["list-checks"], Opt("list-checks", "text"), "text|json", "List the optional checks and what is known about codes"),
        o("", &["dump-ast"], No("dump-ast"), "", "Print each file's parsed AST as JSON instead of the report"),
        o("", &["summary"], No("summary"), "", "Print finding counts and files checked to stderr after the report"),
        o("", &["fail-fast"], No("fail-fast"), "", "Stop after the first file with findings"),
        o("", &["stdin-filename"], Req("stdin-filename"), "PATH", "Report standard input ('-') as PATH"),
        o("", &["output"], Req("output"), "FORMAT:PATH", "Also write a FORMAT report to PATH (repeatable)"),
        o("", &["completions"], Req("completions"), "SHELL", "Print a shell completion script (bash, zsh, fish)"),
        o("", &["cache-dir"], Req("cache-dir"), "PATH", "Reuse results for unchanged files, kept in PATH"),
    ]
}

// "Output format (" ++ formatList ++ ")"
const FORMAT_DESC: &str = "Output format (checkstyle, diff, gcc, json, json1, quiet, sarif, tty)";

/// GetOpt `usageInfo`
pub fn usage_info(header: &str, opts: &[&OptDescr]) -> String {
    let rows: Vec<(String, String, &str)> = opts
        .iter()
        .map(|d| {
            let shorts: Vec<String> = d
                .shorts
                .chars()
                .map(|c| match d.arg {
                    No(_) => format!("-{}", c),
                    Req(_) => format!("-{} {}", c, d.placeholder),
                    Opt(..) => format!("-{}[{}]", c, d.placeholder),
                })
                .collect();
            let longs: Vec<String> = d
                .longs
                .iter()
                .map(|l| match d.arg {
                    No(_) => format!("--{}", l),
                    Req(_) => format!("--{}={}", l, d.placeholder),
                    Opt(..) => format!("--{}[={}]", l, d.placeholder),
                })
                .collect();
            (shorts.join(", "), longs.join(", "), d.desc)
        })
        .collect();
    let w1 = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
    let w2 = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    let mut s = format!("{}\n", header);
    for (a, b, d) in rows {
        s.push_str(&format!("  {:<w1$}  {:<w2$}  {}\n", a, b, d, w1 = w1, w2 = w2));
    }
    s
}

pub fn get_usage_info() -> String {
    let opts = options();
    let refs: Vec<&OptDescr> = opts.iter().collect();
    usage_info(USAGE_HEADER, &refs)
}

enum Opt {
    Opt(Flag),
    Unreq(String),
    NonOpt(String),
    EndOfOpts,
    OptErr(String),
}

fn flag_of(d: &OptDescr, arg: Option<String>) -> Flag {
    match (d.arg, arg) {
        (No(name), _) => Flag(
            name.to_string(),
            match name {
                "sourced" => "false",
                _ => "true",
            }
            .to_string(),
        ),
        (Req(name), Some(a)) => Flag(name.to_string(), a),
        (Opt(name, _), Some(a)) => Flag(name.to_string(), a),
        (Opt(name, default), None) => Flag(name.to_string(), default.to_string()),
        (Req(name), None) => Flag(name.to_string(), String::new()),
    }
}

fn err_ambig(ods: &[&OptDescr], opt_str: &str) -> Opt {
    let header = format!("option `{}' is ambiguous; could be one of:", opt_str);
    Opt::OptErr(usage_info(&header, ods))
}

fn err_req(d: &OptDescr, opt_str: &str) -> Opt {
    Opt::OptErr(format!("option `{}' requires an argument {}\n", opt_str, d.placeholder))
}

/// GetOpt `getOpt Permute`: returns (flags, non-options, errors).
pub fn get_opt(optdescr: &[OptDescr], args: &[String]) -> (Vec<Flag>, Vec<String>, Vec<String>) {
    let mut queue: std::collections::VecDeque<String> = args.iter().cloned().collect();
    let mut flags = vec![];
    let mut nonopts = vec![];
    let mut unrec = vec![];
    let mut errs = vec![];
    while let Some(arg) = queue.pop_front() {
        let rest: Vec<String> = queue.iter().take(1).cloned().collect();
        let (opt, consumed, reinject) = get_next(&arg, &rest, optdescr);
        for _ in 0..consumed {
            queue.pop_front();
        }
        if let Some(r) = reinject {
            // `('-':xs):rest`: the remainder of a short cluster is parsed next.
            queue.push_front(r);
        }
        match opt {
            Opt::Opt(f) => flags.push(f),
            Opt::Unreq(u) => unrec.push(u),
            Opt::NonOpt(x) => nonopts.push(x),
            Opt::EndOfOpts => {
                nonopts.extend(queue.drain(..));
                break;
            }
            Opt::OptErr(e) => errs.push(e),
        }
    }
    // getOpt: es ++ map errUnrec us
    errs.extend(unrec.iter().map(|u| format!("unrecognized option `{}'\n", u)));
    (flags, nonopts, errs)
}

/// Returns (opt, extra args consumed from `rest`, re-injected cluster remainder).
fn get_next(arg: &str, rest: &[String], optdescr: &[OptDescr]) -> (Opt, usize, Option<String>) {
    if arg == "--" {
        return (Opt::EndOfOpts, 0, None);
    }
    if let Some(xs) = arg.strip_prefix("--") {
        return long_opt(xs, rest, optdescr);
    }
    let mut cs = arg.chars();
    if cs.next() == Some('-') {
        if let Some(x) = cs.next() {
            return short_opt(x, cs.as_str(), rest, optdescr);
        }
    }
    (Opt::NonOpt(arg.to_string()), 0, None)
}

fn short_opt(y: char, ys: &str, rs: &[String], optdescr: &[OptDescr]) -> (Opt, usize, Option<String>) {
    let options: Vec<&OptDescr> = optdescr.iter().filter(|d| d.shorts.contains(y)).collect();
    let opt_str = format!("-{}", y);
    match options.as_slice() {
        [] if ys.is_empty() => (Opt::Unreq(opt_str), 0, None),
        [] => (Opt::Unreq(opt_str), 0, Some(format!("-{}", ys))),
        [d] => match d.arg {
            No(_) if ys.is_empty() => (Opt::Opt(flag_of(d, None)), 0, None),
            No(_) => (Opt::Opt(flag_of(d, None)), 0, Some(format!("-{}", ys))),
            Req(_) if ys.is_empty() => match rs.first() {
                None => (err_req(d, &opt_str), 0, None),
                Some(r) => (Opt::Opt(flag_of(d, Some(r.clone()))), 1, None),
            },
            Req(_) => (Opt::Opt(flag_of(d, Some(ys.to_string()))), 0, None),
            Opt(..) if ys.is_empty() => (Opt::Opt(flag_of(d, None)), 0, None),
            Opt(..) => (Opt::Opt(flag_of(d, Some(ys.to_string()))), 0, None),
        },
        many => (err_ambig(many, &opt_str), 0, None),
    }
}

fn long_opt(ls: &str, rs: &[String], optdescr: &[OptDescr]) -> (Opt, usize, Option<String>) {
    let (opt, arg) = match ls.find('=') {
        Some(i) => (&ls[..i], &ls[i..]),
        None => (ls, ""),
    };
    let exact: Vec<&OptDescr> = optdescr.iter().filter(|d| d.longs.contains(&opt)).collect();
    let options: Vec<&OptDescr> = if exact.is_empty() {
        optdescr.iter().filter(|d| d.longs.iter().any(|l| l.starts_with(opt))).collect()
    } else {
        exact
    };
    let opt_str = format!("--{}", opt);
    let unreq = || (Opt::Unreq(format!("--{}", ls)), 0, None);
    match options.as_slice() {
        [d] => match (d.arg, arg) {
            (No(_), "") => (Opt::Opt(flag_of(d, None)), 0, None),
            (No(_), a) if a.starts_with('=') => {
                (Opt::OptErr(format!("option `{}' doesn't allow an argument\n", opt_str)), 0, None)
            }
            (Req(_), "") => match rs.first() {
                None => (err_req(d, &opt_str), 0, None),
                Some(r) => (Opt::Opt(flag_of(d, Some(r.clone()))), 1, None),
            },
            (Req(_), a) if a.starts_with('=') => (Opt::Opt(flag_of(d, Some(a[1..].to_string()))), 0, None),
            (Opt(..), "") => (Opt::Opt(flag_of(d, None)), 0, None),
            (Opt(..), a) if a.starts_with('=') => (Opt::Opt(flag_of(d, Some(a[1..].to_string()))), 0, None),
            _ => unreq(),
        },
        [] => unreq(),
        many => (err_ambig(many, &opt_str), 0, None),
    }
}
