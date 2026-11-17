// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` checks, no Haskell counterpart.
// Owned by L3: variables, += and tests (SC9101 9110 9118 9121 9123 9129 9132).
// Runs once per node, only when a bash target is set. Report with `super::too_new`.

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_core::ast::Id;

pub fn check(_p: &Params<'_>, _t: Id, _c: &mut Comments) {}
