// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/CFG.hs (e11f964): `buildGraph` and the
// `CFM` graph-building monad.

use std::collections::BTreeSet;

use shellcheck_core::ast::{Ast, Id};

use super::{CfgParameters, CfgResult, Node};

/// The `[(Id, Node)]` association list of `CFW`, stored compactly.
#[derive(Clone, Debug, Default)]
pub(crate) struct Association {}

impl Association {
    pub(crate) fn nodes_for(&self, _id: Id) -> Option<BTreeSet<Node>> {
        panic!("unported: cfIdToNodes")
    }
}

/// `buildGraph :: CFGParameters -> Token -> CFGResult`
pub fn build_graph(_params: &CfgParameters, _ast: &Ast, _root: Id) -> CfgResult {
    panic!("unported: buildGraph")
}
