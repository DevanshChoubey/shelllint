// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFGAnalysis.hs (e11f964): the DFA engine.

use shellcheck_core::ast::{Ast, Id};

use super::{CfgAnalysis, CfgParameters};

pub(super) fn analyze_control_flow(params: &CfgParameters, ast: &Ast, root: Id) -> CfgAnalysis {
    // TEMPORARY: graph only; the dataflow is not ported yet.
    let cfg = crate::cfg::build_graph(params, ast, root);
    CfgAnalysis { cfg, node_to_data: Vec::new(), data_base: 0 }
}
