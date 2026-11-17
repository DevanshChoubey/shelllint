// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Owned by L1: builtins and their flags (SC9100 9102 9103 9111 9112 9114 9115 9116 9119 9120 9122 9124 9126 9127 9128 9130 9131 9133 9136).
// Runs once per node, only when a bash target is set. Report with `super::too_new`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::Id;

pub fn check(_p: &Params<'_>, _t: Id, _c: &mut Comments) {}
