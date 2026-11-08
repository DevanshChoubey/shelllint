// SPDX-License-Identifier: GPL-3.0-or-later
//! Embeddable ShellCheck linter.
//!
//! This crate is the stable library facade over the shelllint
//! pipeline. Internal crates (parser, analyzer, formatter, ...) are not
//! re-exported; everything you need is in this crate's root.
//!
//! # Quick start
//!
//! ```
//! use shelllint::{Linter, Severity, Shell};
//!
//! let linter = Linter::builder()
//!     .shell(Shell::Bash)
//!     .severity(Severity::Warning)
//!     .build();
//!
//! let diagnostics = linter.check_str("echo $foo\n");
//! assert!(diagnostics.iter().all(|d| d.level <= Severity::Warning));
//! ```
//!
//! # Concepts
//!
//! * [`Linter`] holds a configuration. It is immutable once built, is
//!   [`Send`] and [`Sync`], and can be shared across threads and reused for
//!   any number of scripts.
//! * [`Linter::check_str`] and [`Linter::check_file`] return
//!   [`Diagnostic`]s in ShellCheck's report order. Columns follow the
//!   `json1` formatter: tabs count as one column.
//! * A [`Diagnostic`] may carry a [`Fix`]. [`Fix::apply`] applies one fix to
//!   a source string, and [`apply_fixes`] applies a whole set the same way
//!   `shellcheck -f diff` does (conflicting fixes are dropped, the rest are
//!   merged).
//! * [`list_checks`] describes the optional checks that can be enabled.
//!
//! # Stack size
//!
//! Parsing recurses once per nesting level, so the checker runs each script
//! on a thread with a large reserved stack (512 MiB of address space, only
//! the touched pages are committed). This is handled internally; callers do
//! not need to spawn threads with a bigger stack.
//!
//! # Configuration files
//!
//! The facade never reads `.shellcheckrc` files and never follows `source`
//! directives, so results depend only on the builder settings and the
//! script. This matches `shellcheck --norc` without `-x`.
//!
//! # Features
//!
//! * `serde`: `Serialize` and `Deserialize` for [`Diagnostic`], [`Fix`],
//!   [`Replacement`], [`InsertionPoint`] and [`Severity`]. The JSON shape is
//!   the `json1` formatter's comment object, minus its `file` key (the facade
//!   has no file in a diagnostic; `check_file` knows the path).

#![deny(missing_docs)]

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use shelllint_analyzer::api::CheckEntry;
use shelllint_checker::{check_script, decode_string};
use shelllint_checks::CHECKS;
use shelllint_core::interface::{
    CheckSpec, Fix as CoreFix, InsertionPoint as CoreInsertionPoint, NewSystemInterface, PositionedComment,
    Severity as CoreSeverity, Shell as CoreShell, empty_check_spec,
};
use shelllint_formatter::fixer::{apply_fix, merge_fixes};
use shelllint_formatter::{haskell_lines, make_non_virtual};

/// The shell dialect a script is written in.
///
/// Passed to [`LinterBuilder::shell`]. Without it, the shell is taken from
/// the shebang, falling back to the file extension and then to POSIX `sh`
/// rules, as the CLI does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shell {
    /// POSIX `sh`.
    Sh,
    /// Bash.
    Bash,
    /// Dash.
    Dash,
    /// Korn shell.
    Ksh,
    /// BusyBox `sh`.
    BusyboxSh,
}

impl From<Shell> for CoreShell {
    fn from(s: Shell) -> CoreShell {
        match s {
            Shell::Sh => CoreShell::Sh,
            Shell::Bash => CoreShell::Bash,
            Shell::Dash => CoreShell::Dash,
            Shell::Ksh => CoreShell::Ksh,
            Shell::BusyboxSh => CoreShell::BusyboxSh,
        }
    }
}

/// How serious a diagnostic is. Ordered from most to least serious.
///
/// Used for both [`Diagnostic::level`] and [`LinterBuilder::severity`]: a
/// minimum severity of `Warning` drops `Info` and `Style` diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Severity {
    /// A definite error.
    Error,
    /// Likely a bug.
    Warning,
    /// Worth knowing about.
    Info,
    /// Style or portability advice.
    Style,
}

impl From<Severity> for CoreSeverity {
    fn from(s: Severity) -> CoreSeverity {
        match s {
            Severity::Error => CoreSeverity::ErrorC,
            Severity::Warning => CoreSeverity::WarningC,
            Severity::Info => CoreSeverity::InfoC,
            Severity::Style => CoreSeverity::StyleC,
        }
    }
}

fn severity_from_core(s: CoreSeverity) -> Severity {
    match s {
        CoreSeverity::ErrorC => Severity::Error,
        CoreSeverity::WarningC => Severity::Warning,
        CoreSeverity::InfoC => Severity::Info,
        CoreSeverity::StyleC => Severity::Style,
    }
}

/// One finding about a script.
///
/// Line and column are 1-based. Columns count a tab as one column, as the
/// `json1` formatter does. `end_line` and `end_column` mark the end of the
/// span.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Diagnostic {
    /// The ShellCheck code, e.g. `2086` for `SC2086`.
    pub code: u32,
    /// The severity of the finding.
    pub level: Severity,
    /// The human-readable message.
    pub message: String,
    /// First line of the span (1-based).
    pub line: usize,
    /// First column of the span (1-based).
    pub column: usize,
    /// Last line of the span (1-based).
    pub end_line: usize,
    /// Last column of the span (1-based).
    pub end_column: usize,
    /// A suggested automatic fix, if one exists.
    pub fix: Option<Fix>,
}

/// Where a replacement is inserted relative to its span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InsertionPoint {
    /// Insert before the span's start (serialised as `beforeStart`).
    #[cfg_attr(feature = "serde", serde(rename = "beforeStart"))]
    BeforeStart,
    /// Insert after the span's end (serialised as `afterEnd`).
    #[cfg_attr(feature = "serde", serde(rename = "afterEnd"))]
    AfterEnd,
}

/// One text replacement that makes up part of a [`Fix`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Replacement {
    /// First line of the replaced span (1-based).
    pub line: usize,
    /// First column of the replaced span (1-based).
    pub column: usize,
    /// Last line of the replaced span (1-based).
    pub end_line: usize,
    /// Last column of the replaced span (1-based).
    pub end_column: usize,
    /// Higher precedence replacements are applied first.
    pub precedence: i64,
    /// The text to insert.
    pub replacement: String,
    /// Whether the text goes before the span or after it.
    pub insertion_point: InsertionPoint,
}

/// An automatic fix for a [`Diagnostic`].
///
/// Apply it with [`Fix::apply`]. The replacements are readable through
/// [`Fix::replacements`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "FixRepr", into = "FixRepr"))]
pub struct Fix {
    replacements: Vec<Replacement>,
    /// The fix in the checker's own coordinates, with tab stops still
    /// present. `apply` runs on this so tabbed sources come out right.
    /// A deserialised fix rebuilds it from `replacements` (tab = one
    /// column) and sets `untabbed`; `raw_for` then restores the tab stops
    /// from the source it is applied to.
    raw: CoreFix,
    untabbed: bool,
}

/// Fixes are equal when their public replacements are; the internal
/// coordinates are derived from them.
impl PartialEq for Fix {
    fn eq(&self, other: &Fix) -> bool {
        self.replacements == other.replacements
    }
}

impl Eq for Fix {}

impl Fix {
    /// The replacements, in the order ShellCheck emitted them.
    pub fn replacements(&self) -> &[Replacement] {
        &self.replacements
    }

    /// Apply this fix alone to `src` and return the new text.
    ///
    /// A missing final newline stays missing only if the fix leaves the last
    /// line alone; otherwise the result ends with a newline, as in
    /// `shellcheck -f diff`. Positions refer to `src`, so pass the same text
    /// that was checked.
    ///
    /// ```
    /// use shelllint::{Linter, apply_fixes};
    ///
    /// let src = "rm $file\n";
    /// let diags = Linter::new().check_str(src);
    /// let fix = diags.iter().find(|d| d.code == 2086).unwrap().fix.as_ref().unwrap();
    /// assert_eq!(fix.apply(src), "rm \"$file\"\n");
    /// assert_eq!(apply_fixes(src, &diags), "rm \"$file\"\n");
    /// ```
    pub fn apply(&self, src: &str) -> String {
        apply_raw(&self.raw_for(src), src)
    }

    /// The fix in checker coordinates (tab stops of 8) for `src`.
    fn raw_for(&self, src: &str) -> CoreFix {
        if !self.untabbed {
            return self.raw.clone();
        }
        let lines = haskell_lines(src);
        let virt = |p: &shelllint_core::interface::Position| {
            let mut p = p.clone();
            let line = usize::try_from(p.line - 1).ok().and_then(|i| lines.get(i));
            if let (Some(line), true) = (line, p.column > 1) {
                // Inverse of the checker's tab-stop removal: the first
                // virtual column of the real column.
                let mut v = 1i64;
                let mut chars = line.chars();
                for _ in 1..p.column {
                    v += match chars.next() {
                        Some('\t') => 8 - ((v - 1) % 8),
                        _ => 1,
                    };
                }
                p.column = v;
            }
            p
        };
        CoreFix {
            replacements: self
                .raw
                .replacements
                .iter()
                .map(|r| shelllint_core::interface::Replacement {
                    start_pos: virt(&r.start_pos),
                    end_pos: virt(&r.end_pos),
                    ..r.clone()
                })
                .collect(),
        }
    }
}

/// Serialisation helper: the `Fix` JSON is `{"replacements": [...]}`.
#[cfg(feature = "serde")]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixRepr {
    replacements: Vec<Replacement>,
}

#[cfg(feature = "serde")]
impl From<Fix> for FixRepr {
    fn from(f: Fix) -> FixRepr {
        FixRepr { replacements: f.replacements }
    }
}

#[cfg(feature = "serde")]
impl From<FixRepr> for Fix {
    fn from(r: FixRepr) -> Fix {
        let replacements = r
            .replacements
            .iter()
            .map(|x| shelllint_core::interface::Replacement {
                start_pos: pos(x.line, x.column),
                end_pos: pos(x.end_line, x.end_column),
                string: x.replacement.clone(),
                precedence: x.precedence,
                insertion_point: match x.insertion_point {
                    InsertionPoint::BeforeStart => CoreInsertionPoint::InsertBefore,
                    InsertionPoint::AfterEnd => CoreInsertionPoint::InsertAfter,
                },
            })
            .collect();
        Fix { replacements: r.replacements, raw: CoreFix { replacements }, untabbed: true }
    }
}

#[cfg(feature = "serde")]
fn pos(line: usize, column: usize) -> shelllint_core::interface::Position {
    shelllint_core::interface::Position { file: String::new(), line: line as i64, column: column as i64 }
}

/// Apply every fix in `diagnostics` to `src`, as `shellcheck -f diff` does.
///
/// Fixes are merged in report order: a fix that overlaps one already kept is
/// dropped. The merged set is then applied at once. Diagnostics without a
/// fix are ignored. Pass the diagnostics produced for exactly `src`.
///
/// ```
/// use shelllint::{Linter, apply_fixes};
///
/// let src = "#!/bin/sh\nfor f in $(ls); do echo $f; done\n";
/// let diags = Linter::new().check_str(src);
/// let fixed = apply_fixes(src, &diags);
/// assert_ne!(fixed, src);
/// ```
pub fn apply_fixes(src: &str, diagnostics: &[Diagnostic]) -> String {
    let fixes: Vec<CoreFix> = diagnostics.iter().filter_map(|d| d.fix.as_ref().map(|f| f.raw_for(src))).collect();
    apply_raw(&merge_fixes(&fixes), src)
}

/// Applies a checker-coordinate fix to `src` the way `shellcheck -f diff`
/// sees it: a missing final newline stays missing only when the fix leaves
/// the last line alone. Otherwise the result ends with a newline.
fn apply_raw(fix: &CoreFix, src: &str) -> String {
    let old = haskell_lines(src);
    let new = apply_fix(fix, &old);
    if new.is_empty() {
        return String::new();
    }
    let keeps_missing_newline =
        !src.is_empty() && !src.ends_with('\n') && old.last().copied() == new.last().map(String::as_str);
    let mut out = new.join("\n");
    if !keeps_missing_newline {
        out.push('\n');
    }
    out
}

/// Description of one optional check, as returned by [`list_checks`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckInfo {
    /// The name passed to [`LinterBuilder::enable`], e.g. `require-variable-braces`.
    pub name: &'static str,
    /// The ShellCheck codes the check can report.
    pub codes: Vec<u32>,
    /// What the check looks for.
    pub description: &'static str,
    /// An example that triggers the check.
    pub positive: &'static str,
    /// The same example written so the check stays quiet.
    pub negative: &'static str,
    /// Whether the check must be enabled explicitly. Every check listed
    /// today is optional; this field is kept so the shape can grow.
    pub optional: bool,
}

/// The optional checks available to [`LinterBuilder::enable`], sorted by name.
///
/// Non-optional checks carry no name or description in the port, so they
/// are not listed.
///
/// ```
/// let checks = shelllint::list_checks();
/// assert!(checks.iter().any(|c| c.name == "require-variable-braces"));
/// ```
pub fn list_checks() -> &'static [CheckInfo] {
    static LIST: OnceLock<Vec<CheckInfo>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut entries: Vec<&CheckEntry> = CHECKS.iter().copied().filter(|e| e.optional.is_some()).collect();
        entries.sort_by(|a, b| a.optional.unwrap().name.cmp(b.optional.unwrap().name));
        let mut seen = HashSet::new();
        entries
            .into_iter()
            .filter_map(|e| {
                let d = e.optional?;
                // Parametrised checks register one entry per argument.
                if !seen.insert(d.name) {
                    return None;
                }
                Some(CheckInfo {
                    name: d.name,
                    codes: d.id.iter().map(|&c| c as u32).collect(),
                    description: d.description,
                    positive: d.positive,
                    negative: d.negative,
                    optional: true,
                })
            })
            .collect()
    })
    .as_slice()
}

/// Builder for a [`Linter`]. Obtained from [`Linter::builder`].
///
/// Every setting has the same default as the CLI with `--norc`.
#[derive(Clone, Debug)]
pub struct LinterBuilder {
    spec: CheckSpec,
}

impl LinterBuilder {
    /// Fixes the shell dialect instead of detecting it.
    pub fn shell(mut self, shell: Shell) -> Self {
        self.spec.shell_type_override = Some(shell.into());
        self
    }

    /// Enables every optional check, like `--enable=all`.
    pub fn enable_all_optional(mut self) -> Self {
        self.spec.optional_checks = vec!["all".to_string()];
        self
    }

    /// Enables optional checks by name (see [`list_checks`]). Repeated calls
    /// add to the set.
    pub fn enable<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.spec.optional_checks.extend(names.into_iter().map(Into::into));
        self
    }

    /// Suppresses the given codes. Repeated calls add to the set.
    pub fn exclude<I>(mut self, codes: I) -> Self
    where
        I: IntoIterator<Item = u32>,
    {
        self.spec.excluded_warnings.extend(codes.into_iter().map(i64::from));
        self
    }

    /// Reports only the given codes. Repeated calls add to the set. Without
    /// any call, every code is reported.
    pub fn include<I>(mut self, codes: I) -> Self
    where
        I: IntoIterator<Item = u32>,
    {
        let codes: Vec<i64> = codes.into_iter().map(i64::from).collect();
        if !codes.is_empty() {
            self.spec.included_warnings.get_or_insert_with(Vec::new).extend(codes);
        }
        self
    }

    /// Drops diagnostics less serious than `min`.
    pub fn severity(mut self, min: Severity) -> Self {
        self.spec.min_severity = min.into();
        self
    }

    /// Finishes the configuration.
    pub fn build(self) -> Linter {
        Linter { spec: self.spec }
    }
}

/// A configured ShellCheck linter.
///
/// Immutable and cheap to share: wrap it in an `Arc` or take a reference
/// from several threads at once.
///
/// ```
/// fn assert_send_sync<T: Send + Sync>() {}
/// assert_send_sync::<shelllint::Linter>();
/// ```
#[derive(Clone, Debug)]
pub struct Linter {
    spec: CheckSpec,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    /// A linter with default settings.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Starts a builder with default settings.
    pub fn builder() -> LinterBuilder {
        LinterBuilder { spec: CheckSpec { ignore_rc: true, ..empty_check_spec() } }
    }

    /// Checks `src` and returns its diagnostics in report order.
    ///
    /// The script is named `-` for shell-detection purposes, so a file
    /// extension cannot select the shell.
    ///
    /// ```
    /// use shelllint::Linter;
    ///
    /// let diags = Linter::new().check_str("echo $1\n");
    /// assert!(diags.iter().any(|d| d.code == 2086));
    /// ```
    pub fn check_str(&self, src: &str) -> Vec<Diagnostic> {
        self.run("-", src.to_string())
    }

    /// Reads `path` and checks its contents.
    ///
    /// The file is decoded as UTF-8, falling back to ISO-8859-1 for invalid
    /// bytes, as the CLI does. The path also selects the shell from its
    /// extension (`.bash`, `.ksh`, `.dash`, ...) when no shebang or
    /// [`LinterBuilder::shell`] decides it.
    ///
    /// # Errors
    ///
    /// Returns the I/O error if the file cannot be read.
    pub fn check_file(&self, path: impl AsRef<Path>) -> io::Result<Vec<Diagnostic>> {
        let path = path.as_ref();
        let script = decode_string(&std::fs::read(path)?);
        Ok(self.run(&path.to_string_lossy(), script))
    }

    fn run(&self, filename: &str, script: String) -> Vec<Diagnostic> {
        let spec = CheckSpec { filename: filename.to_string(), script: script.clone(), ..self.spec.clone() };
        // `check_script` runs the parser on a thread with a large stack.
        let result = check_script(&NewSystemInterface, &spec);
        let untabbed = make_non_virtual(&result.comments, &script);
        result.comments.iter().zip(untabbed.iter()).map(|(raw, pc)| to_diagnostic(pc, raw.fix.as_ref())).collect()
    }
}

/// Builds a diagnostic from an untabbed comment and its checker-coordinate fix.
fn to_diagnostic(pc: &PositionedComment, raw_fix: Option<&CoreFix>) -> Diagnostic {
    let fix = match (&pc.fix, raw_fix) {
        (Some(public), Some(raw)) => Some(Fix {
            replacements: public
                .replacements
                .iter()
                .map(|r| Replacement {
                    line: r.start_pos.line.max(0) as usize,
                    column: r.start_pos.column.max(0) as usize,
                    end_line: r.end_pos.line.max(0) as usize,
                    end_column: r.end_pos.column.max(0) as usize,
                    precedence: r.precedence,
                    replacement: r.string.clone(),
                    insertion_point: match r.insertion_point {
                        CoreInsertionPoint::InsertBefore => InsertionPoint::BeforeStart,
                        CoreInsertionPoint::InsertAfter => InsertionPoint::AfterEnd,
                    },
                })
                .collect(),
            raw: raw.clone(),
            untabbed: false,
        }),
        _ => None,
    };
    Diagnostic {
        code: pc.comment.code.max(0) as u32,
        level: severity_from_core(pc.comment.severity),
        message: pc.comment.message.clone(),
        line: pc.start_pos.line.max(0) as usize,
        column: pc.start_pos.column.max(0) as usize,
        end_line: pc.end_pos.line.max(0) as usize,
        end_column: pc.end_pos.column.max(0) as usize,
        fix,
    }
}
