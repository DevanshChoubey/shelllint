// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Owned by L2: ${..} and brace expansions (SC9107 9108 9117 9125 9134 9135).
// Runs once per node, only when a bash target is set. Report with `super::too_new`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::Id;

pub fn check(_p: &Params<'_>, _t: Id, _c: &mut Comments) {}
