// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964)
//
// The parser is split by Parser.hs line ranges: `engine` (the Parsec
// semantics Parser.hs runs on), `state` (lines 156-463), and one `area_*`
// module per range of readers.
#![forbid(unsafe_code)]

use shelllint_core::ast::{Ast, Id};
use shelllint_core::interface::{ParseSpec, Position, PositionedComment, SystemInterface};

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
mod rcdir;

/// Interface.hs `ParseResult`, holding the arena instead of
/// `prTokenPositions` (the spans live in `ast`).
// DIVERGENCE: `prTokenPositions` maps each id to a `Position` pair that
// carries its file name (`posFile`); the arena spans have no file, so the
// file of each span is kept here as an index into `files`
// (see `token_positions`).
#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub comments: Vec<PositionedComment>,
    pub ast: Ast,
    pub root: Option<Id>,
    /// File (start, end) of each token's span, as indexes into `files`;
    /// parallel to `ast`.
    pub span_files: Vec<(u32, u32)>,
    /// File names (Parsec's `SourceName`s) seen while parsing.
    pub files: Vec<String>,
}

impl ParseResult {
    /// `Map.lookup id prTokenPositions`
    pub fn token_positions(&self, id: Id) -> Option<(Position, Position)> {
        let &(sf, ef) = self.span_files.get(id.0 as usize)?;
        let (s, e) = self.ast.span(id);
        let pos = |f: u32, line: u32, column: u32| Position {
            file: self.files[f as usize].clone(),
            line: line as i64,
            column: column as i64,
        };
        Some((pos(sf, s.line, s.column), pos(ef, e.line, e.column)))
    }
}

/// Interface.hs `newParseResult`
pub fn new_parse_result() -> ParseResult {
    ParseResult::default()
}

/// `parseScript`
pub fn parse_script(sys: &dyn SystemInterface, spec: &ParseSpec) -> ParseResult {
    area_toplevel::parse_script(sys, spec)
}
