// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1612-1835:
// all $ expansions, variable names.
//
// The `prop_` tests whose parser is a plain reader run through the
// conformance harness (`report --kind parse`). The ones below are the
// `prop_readDollarVariable2..5` combinator expressions the corpus could not
// parse, ported by hand, plus checks of the tokens and Ids readDollarVariable
// builds.

use shellcheck_core::ast::{Id, Token};
use shellcheck_parser::engine::{P, PR};
use shellcheck_parser::testing::{self, is_ok, is_warning, named_parser};

fn read_dollar_variable(p: &mut P<'_>) -> PR<()> {
    (named_parser("readDollarVariable").unwrap())(p)
}

// prop_readDollarVariable2 = isOk (readDollarVariable >> anyChar) "$?!"
#[test]
fn prop_read_dollar_variable2() {
    assert!(is_ok(
        |p| {
            read_dollar_variable(p)?;
            p.any_char().map(drop)
        },
        "$?!"
    ));
}

// prop_readDollarVariable3 = isWarning (readDollarVariable >> anyChar) "$10"
#[test]
fn prop_read_dollar_variable3() {
    assert!(is_warning(
        |p| {
            read_dollar_variable(p)?;
            p.any_char().map(drop)
        },
        "$10"
    ));
}

// prop_readDollarVariable4 = isWarning (readDollarVariable >> string "[@]") "$arr[@]"
#[test]
fn prop_read_dollar_variable4() {
    assert!(is_warning(
        |p| {
            read_dollar_variable(p)?;
            p.string("[@]").map(drop)
        },
        "$arr[@]"
    ));
}

// prop_readDollarVariable5 = isWarning (readDollarVariable >> string "[f") "$arr[f"
#[test]
fn prop_read_dollar_variable5() {
    assert!(is_warning(
        |p| {
            read_dollar_variable(p)?;
            p.string("[f").map(drop)
        },
        "$arr[f"
    ));
}

/// `$name` builds `T_DollarBraced id3 False (T_NormalWord id1 [T_Literal id2 name])`
/// with the word's ids allocated first, as `wrapString` does.
fn check_dollar_variable_tokens(input: &str, name: &str, end_col: u32) {
    let (r, user, _) = testing::run_test_parser(read_dollar_variable, input);
    assert!(r.is_ok(), "{r:?}");
    let ast = &user.ast;
    assert_eq!(ast.len(), 3);
    assert_eq!(*ast.get(Id(0)), Token::T_NormalWord(vec![Id(1)]));
    assert_eq!(*ast.get(Id(1)), Token::T_Literal(name.to_string()));
    assert_eq!(*ast.get(Id(2)), Token::T_DollarBraced(false, Id(0)));
    let (s, e) = ast.span(Id(2));
    assert_eq!((s.line, s.column, e.line, e.column), (1, 1, 1, end_col));
    let (s, e) = ast.span(Id(0));
    assert_eq!((s.column, e.column), (2, end_col));
}

#[test]
fn dollar_variable_regular_tokens() {
    check_dollar_variable_tokens("$foo_1", "foo_1", 7);
}

#[test]
fn dollar_variable_positional_tokens() {
    check_dollar_variable_tokens("$1", "1", 3);
}

#[test]
fn dollar_variable_special_tokens() {
    check_dollar_variable_tokens("$@", "@", 3);
}
