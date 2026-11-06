// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964)
//
// The parser is split by Parser.hs line ranges: `engine` (the Parsec
// semantics Parser.hs runs on), `state` (lines 156-463), and one `area_*`
// module per range of readers.
#![forbid(unsafe_code)]

use shellcheck_core::ast::{Ast, Id};
use shellcheck_core::interface::{ParseSpec, PositionedComment, SystemInterface};

pub mod engine;
pub mod state;
pub mod testing;

mod area_arith;
mod area_assign;
mod area_commands;
mod area_compound;
mod area_condition;
mod area_dollar;
mod area_redirect;
mod area_spacing;
mod area_toplevel;
mod area_words;

/// Interface.hs `ParseResult`, holding the arena instead of
/// `prTokenPositions` (the spans live in `ast`).
#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub comments: Vec<PositionedComment>,
    pub ast: Ast,
    pub root: Option<Id>,
}

/// Interface.hs `newParseResult`
pub fn new_parse_result() -> ParseResult {
    ParseResult::default()
}

/// `parseScript`
pub fn parse_script(sys: &dyn SystemInterface, spec: &ParseSpec) -> ParseResult {
    area_toplevel::parse_script(sys, spec)
}
