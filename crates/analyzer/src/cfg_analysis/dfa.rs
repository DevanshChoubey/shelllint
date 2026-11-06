// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFGAnalysis.hs (e11f964): the DFA engine.

use shellcheck_core::ast::{Ast, Id};

use super::{CfgAnalysis, CfgParameters};

pub(super) fn analyze_control_flow(_params: &CfgParameters, _ast: &Ast, _root: Id) -> CfgAnalysis {
    panic!("unported: analyzeControlFlow")
}
