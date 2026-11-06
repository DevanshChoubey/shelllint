// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analyzer.hs and AnalyzerLib.hs (e11f964)
// ASTLib lives in shellcheck-core (the parser needs it too, as Parser.hs imports ASTLib); re-exported here.
#![forbid(unsafe_code)]
pub use shellcheck_core::astlib;

pub mod analyzerlib;
pub mod api;
pub mod cfg;
pub mod cfg_analysis;
pub mod testing;
