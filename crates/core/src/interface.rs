// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Interface.hs (e11f964)

use crate::ast::Id;

/// `SystemInterface m`. The default methods are `newSystemInterface`.
// DIVERGENCE: Haskell's record is parametrised over a monad; here it is a
// synchronous trait. It requires `Sync` so the checker can hand
// `&dyn SystemInterface` to the big-stack thread it parses on.
pub trait SystemInterface: Sync {
    /// Given:
    ///   What annotations say about including external files (if anything)
    ///   A resolved filename from siFindSource
    ///   Read the file or return an error
    fn read_file(&self, _external: Option<bool>, _path: &str) -> Result<String, ErrorMessage> {
        Err("Not implemented".to_string())
    }

    /// Given:
    ///   the current script,
    ///   what annotations say about including external files (if anything)
    ///   a list of source-path annotations in effect,
    ///   and a sourced file,
    ///   find the sourced file
    fn find_source(
        &self,
        _current: &str,
        _external: Option<bool>,
        _source_paths: &[String],
        name: &str,
    ) -> String {
        name.to_string()
    }

    /// Get the configuration file (name, contents) for a filename
    fn get_config(&self, _file: &str) -> Option<(String, String)> {
        None
    }
}

// ShellCheck input and output
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckSpec {
    pub filename: String,
    pub script: String,
    pub check_sourced: bool,
    pub ignore_rc: bool,
    pub excluded_warnings: Vec<Code>,
    pub included_warnings: Option<Vec<Code>>,
    pub shell_type_override: Option<Shell>,
    pub min_severity: Severity,
    pub extended_analysis: Option<bool>,
    pub optional_checks: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckResult {
    pub filename: String,
    pub comments: Vec<PositionedComment>,
}

pub fn empty_check_result() -> CheckResult {
    CheckResult {
        filename: String::new(),
        comments: vec![],
    }
}

pub fn empty_check_spec() -> CheckSpec {
    CheckSpec {
        filename: String::new(),
        script: String::new(),
        check_sourced: false,
        ignore_rc: false,
        excluded_warnings: vec![],
        included_warnings: None,
        shell_type_override: None,
        min_severity: Severity::StyleC,
        extended_analysis: None,
        optional_checks: vec![],
    }
}

pub fn new_parse_spec() -> ParseSpec {
    ParseSpec {
        filename: String::new(),
        script: String::new(),
        check_sourced: false,
        ignore_rc: false,
        shell_type_override: None,
    }
}

/// `newSystemInterface`: every method has its default behaviour.
#[derive(Clone, Copy, Debug, Default)]
pub struct NewSystemInterface;

impl SystemInterface for NewSystemInterface {}

pub fn new_system_interface() -> NewSystemInterface {
    NewSystemInterface
}

// Parser input and output
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseSpec {
    pub filename: String,
    pub script: String,
    pub check_sourced: bool,
    pub ignore_rc: bool,
    pub shell_type_override: Option<Shell>,
}

// DIVERGENCE: `ParseResult` / `newParseResult` are defined by the parser crate,
// since they carry the arena `Ast`.

// Analyzer input and output
// DIVERGENCE: `asScript` and `asTokenPositions` are not stored here; the arena
// `Ast` (which holds the spans) and the root `Id` are passed separately.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AnalysisSpec {
    pub shell_type: Option<Shell>,
    pub fallback_shell: Option<Shell>,
    pub execution_mode: ExecutionMode,
    pub check_sourced: bool,
    pub optional_checks: Vec<String>,
    pub extended_analysis: Option<bool>,
}

pub fn new_analysis_spec() -> AnalysisSpec {
    AnalysisSpec {
        shell_type: None,
        fallback_shell: None,
        execution_mode: ExecutionMode::Executed,
        check_sourced: false,
        optional_checks: vec![],
        extended_analysis: None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AnalysisResult {
    pub comments: Vec<TokenComment>,
}

pub fn new_analysis_result() -> AnalysisResult {
    AnalysisResult { comments: vec![] }
}

// Formatter options
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormatterOptions {
    pub color_option: ColorOption,
    pub wiki_link_count: i64,
}

pub fn new_formatter_options() -> FormatterOptions {
    FormatterOptions {
        color_option: ColorOption::ColorAuto,
        wiki_link_count: 3,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckDescription {
    pub name: String,
    pub id: Vec<Code>,
    pub description: String,
    pub positive: String,
    pub negative: String,
}

pub fn new_check_description() -> CheckDescription {
    CheckDescription {
        name: String::new(),
        id: vec![],
        description: String::new(),
        positive: String::new(),
        negative: String::new(),
    }
}

// Supporting data types
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Shell {
    Ksh,
    Sh,
    Bash,
    Dash,
    BusyboxSh,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExecutionMode {
    Executed,
    Sourced,
}

pub type ErrorMessage = String;
// DIVERGENCE: Haskell `Integer` (unbounded) becomes `i64`.
pub type Code = i64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Severity {
    ErrorC,
    WarningC,
    InfoC,
    StyleC,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Position {
    /// Filename
    pub file: String,
    /// 1 based source line
    pub line: i64,
    /// 1 based source column, where tabs are 8
    pub column: i64,
}

pub fn new_position() -> Position {
    Position {
        file: String::new(),
        line: 1,
        column: 1,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Comment {
    pub severity: Severity,
    pub code: Code,
    pub message: String,
}

pub fn new_comment() -> Comment {
    Comment {
        severity: Severity::StyleC,
        code: 0,
        message: String::new(),
    }
}

/// only support single line for now
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Replacement {
    pub start_pos: Position,
    pub end_pos: Position,
    pub string: String,
    /// Order in which the replacements should happen: highest precedence first.
    pub precedence: i64,
    /// Whether to insert immediately before or immediately after the specified region.
    pub insertion_point: InsertionPoint,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InsertionPoint {
    InsertBefore,
    InsertAfter,
}

pub fn new_replacement() -> Replacement {
    Replacement {
        start_pos: new_position(),
        end_pos: new_position(),
        string: String::new(),
        precedence: 1,
        insertion_point: InsertionPoint::InsertAfter,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fix {
    pub replacements: Vec<Replacement>,
}

pub fn new_fix() -> Fix {
    Fix {
        replacements: vec![],
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PositionedComment {
    pub start_pos: Position,
    pub end_pos: Position,
    pub comment: Comment,
    pub fix: Option<Fix>,
}

pub fn new_positioned_comment() -> PositionedComment {
    PositionedComment {
        start_pos: new_position(),
        end_pos: new_position(),
        comment: new_comment(),
        fix: None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TokenComment {
    pub id: Id,
    pub comment: Comment,
    pub fix: Option<Fix>,
}

pub fn new_token_comment() -> TokenComment {
    TokenComment {
        id: Id(0),
        comment: new_comment(),
        fix: None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ColorOption {
    ColorAuto,
    ColorAlways,
    ColorNever,
}

// For testing
/// `mockedSystemInterface files`, optionally modified by `mockRcFile`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MockedSystemInterface {
    pub files: Vec<(String, String)>,
    pub rc: Option<String>,
}

pub fn mocked_system_interface(files: Vec<(String, String)>) -> MockedSystemInterface {
    MockedSystemInterface { files, rc: None }
}

impl SystemInterface for MockedSystemInterface {
    fn read_file(&self, _external: Option<bool>, file: &str) -> Result<String, ErrorMessage> {
        match self.files.iter().find(|(name, _)| name == file) {
            None => Err("File not included in mock.".to_string()),
            Some((_, contents)) => Ok(contents.clone()),
        }
    }

    fn find_source(
        &self,
        _current: &str,
        _external: Option<bool>,
        _source_paths: &[String],
        file: &str,
    ) -> String {
        file.to_string()
    }

    fn get_config(&self, _file: &str) -> Option<(String, String)> {
        self.rc
            .as_ref()
            .map(|rc| (".shellcheckrc".to_string(), rc.clone()))
    }
}

pub fn mock_rc_file(rcfile: &str, mock: MockedSystemInterface) -> MockedSystemInterface {
    MockedSystemInterface {
        rc: Some(rcfile.to_string()),
        ..mock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mocked_system_interface_behaviour() {
        let mock = mocked_system_interface(vec![("lib.sh".into(), "echo hi".into())]);
        assert_eq!(mock.read_file(None, "lib.sh"), Ok("echo hi".to_string()));
        assert_eq!(
            mock.read_file(None, "other.sh"),
            Err("File not included in mock.".to_string())
        );
        assert_eq!(mock.find_source("a", None, &[], "lib.sh"), "lib.sh");
        assert_eq!(mock.get_config("a"), None);
        let mock = mock_rc_file("disable=SC2086", mock);
        assert_eq!(
            mock.get_config("a"),
            Some((".shellcheckrc".to_string(), "disable=SC2086".to_string()))
        );
    }

    #[test]
    fn new_system_interface_defaults() {
        let si = new_system_interface();
        assert_eq!(si.read_file(None, "x"), Err("Not implemented".to_string()));
        assert_eq!(si.find_source("a", None, &[], "b"), "b");
        assert_eq!(si.get_config("a"), None);
    }
}
