// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 3454-3483:
// testEnvironment, isOk/isWarning/isNotOk, getParseOutput, parsesCleanly,
// plus a name -> reader table for the conformance harness.

use shellcheck_core::interface::MockedSystemInterface;

use crate::engine::{P, PR, ParseError};
use crate::state::{Environment, ParseNote, SystemState, UserState};

/// `mockedSystemInterface []`
static NO_FILES: MockedSystemInterface = MockedSystemInterface { files: Vec::new(), rc: None };

/// `testEnvironment`
pub fn test_environment() -> Environment<'static> {
    Environment {
        system_interface: &NO_FILES,
        check_sourced: false,
        current_filename: "myscript".to_string(),
        ignore_rc: false,
        shell_type_override: None,
    }
}

/// Run `parser` on `string` in `testEnvironment` (file name "-"), returning
/// its result and the final user and system states.
pub fn run_test_parser<T>(
    parser: impl FnOnce(&mut P<'_>) -> PR<T>,
    string: &str,
) -> (Result<T, ParseError>, UserState, SystemState) {
    let mut p = P::new(test_environment(), "-", string);
    let r = p.run(parser);
    (r, p.user, p.sys)
}

/// The string parses with no warnings
pub fn is_ok(p: fn(&mut P<'_>) -> PR<()>, s: &str) -> bool {
    parses_cleanly(p, s) == Some(true)
}

/// The string parses with warnings
pub fn is_warning(p: fn(&mut P<'_>) -> PR<()>, s: &str) -> bool {
    parses_cleanly(p, s) == Some(false)
}

/// The string does not parse
pub fn is_not_ok(p: fn(&mut P<'_>) -> PR<()>, s: &str) -> bool {
    parses_cleanly(p, s).is_none()
}

/// If the parser matches the string, return Right [ParseNotes+ParseProblems]
/// If it does not match the string,  return Left  [ParseProblems]
pub fn get_parse_output(
    parser: impl FnOnce(&mut P<'_>) -> PR<()>,
    string: &str,
) -> Result<Vec<ParseNote>, Vec<ParseNote>> {
    let (res, user, sys) = run_test_parser(
        |p| {
            parser(p)?;
            p.eof()
        },
        string,
    );
    // DIVERGENCE: notes and problems are stored oldest first; Haskell lists
    // are newest first. Only emptiness is inspected by the callers.
    match res {
        Ok(()) => Ok(user.parse_notes.into_iter().chain(sys.parse_problems).collect()),
        Err(_) => Err(sys.parse_problems),
    }
}

/// If the parser matches the string, return Just whether it was clean (without emitting suggestions)
/// Otherwise, Nothing
pub fn parses_cleanly(parser: fn(&mut P<'_>) -> PR<()>, string: &str) -> Option<bool> {
    match get_parse_output(parser, string) {
        Ok(list) => Some(list.is_empty()),
        Err(_) => None,
    }
}

/// The reader a `kind: parse` corpus record names as its `target` (the
/// Haskell expression, e.g. `"readNormalWord"` or `"(readBackTicked False)"`),
/// with its result discarded.
pub fn named_parser(name: &str) -> Option<fn(&mut P<'_>) -> PR<()>> {
    let f: fn(&mut P<'_>) -> PR<()> = match name {
        "(readAnnotationWithoutPrefix False)" => |p| p.read_annotation_without_prefix(false).map(drop),
        "(readBackTicked False)" => |p| p.read_back_ticked(false).map(drop),
        "allspacing" => |p| p.allspacing().map(drop),
        "readAndOr" => |p| p.read_and_or().map(drop),
        "readAnnotation" => |p| p.read_annotation().map(drop),
        "readAnyComment" => |p| p.read_any_comment().map(drop),
        "readArithmeticContents" => |p| p.read_arithmetic_contents().map(drop),
        "readArithmeticExpression" => |p| p.read_arithmetic_expression().map(drop),
        "readAssignmentWord" => |p| p.read_assignment_word().map(drop),
        "readBatsTest" => |p| p.read_bats_test().map(drop),
        "readBraceGroup" => |p| p.read_brace_group().map(drop),
        "readBraced" => |p| p.read_braced().map(drop),
        "readCaseClause" => |p| p.read_case_clause().map(drop),
        "readCoProc" => |p| p.read_co_proc().map(drop),
        "readCompoundCommand" => |p| p.read_compound_command().map(drop),
        "readCondition" => |p| p.read_condition().map(drop),
        "readConditionCommand" => |p| p.read_condition_command().map(drop),
        "readConfigKVs" => |p| p.read_config_kvs().map(drop),
        "readDollarArithmetic" => |p| p.read_dollar_arithmetic().map(drop),
        "readDollarBraceCommandExpansion" => |p| p.read_dollar_brace_command_expansion().map(drop),
        "readDollarBraced" => |p| p.read_dollar_braced().map(drop),
        "readDollarDoubleQuote" => |p| p.read_dollar_double_quote().map(drop),
        "readDollarExpansion" => |p| p.read_dollar_expansion().map(drop),
        "readDollarExpression" => |p| p.read_dollar_expression().map(drop),
        "readDollarSingleQuote" => |p| p.read_dollar_single_quote().map(drop),
        "readDollarVariable" => |p| p.read_dollar_variable().map(drop),
        "readDoubleQuoted" => |p| p.read_double_quoted().map(drop),
        "readExtglob" => |p| p.read_extglob().map(drop),
        "readForClause" => |p| p.read_for_clause().map(drop),
        "readFunctionDefinition" => |p| p.read_function_definition().map(drop),
        "readGlob" => |p| p.read_glob().map(drop),
        "readHereString" => |p| p.read_here_string().map(drop),
        "readIfClause" => |p| p.read_if_clause().map(drop),
        "readIoFile" => |p| p.read_io_file().map(drop),
        "readIoRedirect" => |p| p.read_io_redirect().map(drop),
        "readNormalWord" => |p| p.read_normal_word().map(drop),
        "readPipeline" => |p| p.read_pipeline().map(drop),
        "readProcSub" => |p| p.read_proc_sub().map(drop),
        "readScript" => |p| p.read_script().map(drop),
        "readSelectClause" => |p| p.read_select_clause().map(drop),
        "readShebang" => |p| p.read_shebang().map(drop),
        "readSimpleCommand" => |p| p.read_simple_command().map(drop),
        "readSingleQuoted" => |p| p.read_single_quoted().map(drop),
        "readSubshell" => |p| p.read_subshell().map(drop),
        "readTerm" => |p| p.read_term().map(drop),
        "readUntilClause" => |p| p.read_until_clause().map(drop),
        "readWhileClause" => |p| p.read_while_clause().map(drop),
        "spacing" => |p| p.spacing().map(drop),
        _ => return None,
    };
    Some(f)
}
