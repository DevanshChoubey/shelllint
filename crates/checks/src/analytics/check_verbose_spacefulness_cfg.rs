// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkVerboseSpacefulnessCfg`, lines 2299-2300 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_core::ast::Id;

use crate::support::check_spacefulness_cfg_prime;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkVerboseSpacefulnessCfg",
    registry: "optional-tree",
    order: 0,
    optional: Some(StaticCheckDescription {
        name: "quote-safe-variables",
        id: &[2248],
        description: "Suggest quoting variables without metacharacters",
        positive: "var=hello; echo $var",
        negative: "var=hello; echo \"$var\"",
    }),
    kind: CheckKind::Node(check_verbose_spacefulness_cfg),
};

fn check_verbose_spacefulness_cfg(params: &Params<'_>, t: Id, c: &mut Comments) {
    check_spacefulness_cfg_prime(false, params, t, c)
}

#[cfg(test)]
mod tests {
    use shellcheck_analyzer::testing::run_check;

    fn codes(script: &str) -> Vec<u32> {
        run_check("Analytics.hs", "checkVerboseSpacefulnessCfg", "node", script)
            .expect("check is registered")
            .into_iter()
            .map(|c| c as u32)
            .collect()
    }

    #[test]
    fn prop_check_spacefulness_cfg37v() {
        assert!(!codes("@test 'status' {\n [ $status -eq 0 ]\n}").is_empty());
    }
}
