// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkVerboseSpacefulnessCfg`, lines 2299-2300 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params, is_counting_reference, is_param_to, is_quote_free, is_quoted_alternative_reference, used_as_command_name};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind, StaticCheckDescription};
use shellcheck_analyzer::cfg_analysis::{CFVariableProp, SpaceStatus, VariableState, get_incoming_state};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::{get_braced_modifier, get_braced_reference, get_path, is_array_expansion, oversimplify};
use shellcheck_core::data::SPECIAL_VARIABLES_WITHOUT_SPACES;

use crate::support::surround_with;

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

// Don't suggest quotes if this will instead be autocorrected
// from $foo=bar to foo=bar. This is not pretty but ok.
fn quotes_may_conflict_with_sc2281(params: &Params<'_>, t: Id) -> bool {
    let path = get_path(params.ast, t);
    let [_, w, sc, ..] = &path[..] else { return false };
    let Token::T_NormalWord(parts) = params.ast.get(*w) else { return false };
    let [me, lit, ..] = &parts[..] else { return false };
    let Token::T_Literal(s) = params.ast.get(*lit) else { return false };
    if !s.starts_with('=') {
        return false;
    }
    let Token::T_SimpleCommand(_, words) = params.ast.get(*sc) else { return false };
    let Some(cmd) = words.first() else { return false };
    t == *me && *w == *cmd
}

fn add_double_quotes_around(params: &Params<'_>, token: Id) -> shellcheck_core::interface::Fix {
    surround_with(token, params, "\"")
}

fn check_spacefulness_cfg_prime(dirty_pass: bool, params: &Params<'_>, token: Id, c: &mut Comments) {
    let Token::T_DollarBraced(_, list) = params.ast.get(token) else { return };
    let id = token;
    let braced_string = oversimplify(params.ast, *list).concat();
    let name = get_braced_reference(&braced_string);

    let needs_quoting = !is_array_expansion(params.ast, token) // There's another warning for this
        && !is_counting_reference(params.ast, token)
        && !is_quote_free(params.ast, params.shell_type, token)
        && !is_quoted_alternative_reference(params.ast, token)
        && !used_as_command_name(params.ast, token);

    let is_clean_state = |state: &VariableState| {
        state.variable_properties.all_have(CFVariableProp::CFVPInteger)
            || state.variable_value.space_status == SpaceStatus::SpaceStatusClean
    };

    let is_clean = (|| {
        let cfga = params.cfg_analysis.as_deref()?;
        let state = get_incoming_state(cfga, id)?;
        let value = state.variable_in_scope(&name)?;
        Some(is_clean_state(&value))
    })()
    .unwrap_or(false);

    let is_default_assignment = |token: Id| {
        let modifier = get_braced_modifier(&braced_string);
        ["=", ":="].iter().any(|p| modifier.starts_with(p)) && is_param_to(params.ast, ":", token)
    };

    if needs_quoting && (dirty_pass == !is_clean) {
        if !(SPECIAL_VARIABLES_WITHOUT_SPACES.contains(&name.as_str())
            || quotes_may_conflict_with_sc2281(params, token))
        {
            if dirty_pass {
                if is_default_assignment(token) {
                    c.info(token, 2223, "This default assignment may cause DoS due to globbing. Quote it.");
                } else {
                    c.info_with_fix(
                        id,
                        2086,
                        "Double quote to prevent globbing and word splitting.",
                        add_double_quotes_around(params, token),
                    );
                }
            } else {
                c.style_with_fix(
                    id,
                    2248,
                    "Prefer double quoting even when variables don't contain special characters.",
                    add_double_quotes_around(params, token),
                );
            }
        }
    }
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
    #[ignore = "needs cfg engine bodies"]
    fn prop_check_spacefulness_cfg37v() {
        assert!(!codes("@test 'status' {\n [ $status -eq 0 ]\n}").is_empty());
    }
}
