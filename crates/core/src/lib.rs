// SPDX-License-Identifier: GPL-3.0-or-later
// Crate root for the ports of ShellCheck src/ShellCheck/{AST,Interface,Data,Regex,Prelude}.hs (e11f964)
#![forbid(unsafe_code)]

pub mod ast;
pub mod astlib;
pub mod data;
pub mod extras;
pub mod interface;
pub mod prelude;
pub mod regex;
pub mod unicode;
mod unicode_tables;
