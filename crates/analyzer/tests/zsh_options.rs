// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint extension. Tests of the zsh option-state tracker
// (`shelllint_analyzer::zsh_options`, read through `Params::zsh_option`).
//
// Layer 3 of the zsh test plan: a table, one row per spelling and per scope
// rule. A row is a zsh script with `probe <label>` calls. Every expected state
// was taken from real zsh 5.9 (`/usr/bin/zsh -fc`), and `rows_match_real_zsh`
// re-runs every row under zsh whenever `/usr/bin/zsh` exists, so a wrong
// expectation fails there rather than going unnoticed.
//
// A state is written as its difference from the zsh defaults, in the order of
// `ZshOpt` (`+x` on although off by default, `-nomatch` off although on by
// default); "" is the defaults. The recorded zsh output of a row sits in the
// comment above it.
//
// Rows whose tracker answer differs on purpose from what zsh prints carry a
// `zsh:` override with zsh's own answer and say why. They are the static
// rules of the design: a function sees the options at its definition, not at
// its call, and branches are walked as if they all ran.
use shelllint_analyzer::analyzerlib::{Params, ZshOpt, make_parameters};
use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::astlib::get_literal_string;
use shelllint_core::interface::{Shell, new_analysis_spec, new_parse_spec, new_system_interface};
use std::collections::BTreeMap;

const ALL: [(&str, ZshOpt); 14] = [
    ("shwordsplit", ZshOpt::ShWordSplit),
    ("ksharrays", ZshOpt::KshArrays),
    ("globsubst", ZshOpt::GlobSubst),
    ("rcexpandparam", ZshOpt::RcExpandParam),
    ("bsdecho", ZshOpt::BsdEcho),
    ("promptsubst", ZshOpt::PromptSubst),
    ("extendedglob", ZshOpt::ExtendedGlob),
    ("kshglob", ZshOpt::KshGlob),
    ("shglob", ZshOpt::ShGlob),
    ("nullglob", ZshOpt::NullGlob),
    ("nomatch", ZshOpt::NoMatch),
    ("localoptions", ZshOpt::LocalOptions),
    ("errreturn", ZshOpt::ErrReturn),
    ("pipefail", ZshOpt::PipeFail),
];

struct Parsed {
    ast: Ast,
    root: Id,
}

fn parse(script: &str, shell: Shell) -> Parsed {
    let mut spec = new_parse_spec();
    spec.script = script.to_string();
    spec.shell_type_override = Some(shell);
    let mut pr = shelllint_parser::parse_script(&new_system_interface(), &spec);
    let root = pr.root.unwrap_or_else(|| panic!("script does not parse: {script:?}"));
    pr.ast.compute_parents(root);
    Parsed { ast: pr.ast, root }
}

fn make_params(p: &Parsed, shell: Shell) -> Params<'_> {
    let mut spec = new_analysis_spec();
    spec.shell_type = Some(shell);
    make_parameters(&spec, &p.ast, p.root)
}

/// The `probe <label>` commands in source order: label and node.
fn probes(ast: &Ast, root: Id) -> Vec<(String, Id)> {
    let mut out = Vec::new();
    ast.analyze(
        root,
        &mut |id| {
            if let Token::T_SimpleCommand(_, words) = ast.get(id) {
                if words.len() == 2 && get_literal_string(ast, words[0]).as_deref() == Some("probe") {
                    out.push((get_literal_string(ast, words[1]).expect("literal probe label"), id));
                }
            }
        },
        &mut |_| {},
    );
    out
}

/// `+opt -opt` against the zsh defaults, in `ZshOpt` order, each with a
/// leading space (so "" is the defaults and the zsh side can build the same).
fn diff(on: impl Fn(ZshOpt) -> bool) -> String {
    let mut s = String::new();
    for (name, opt) in ALL {
        let default = opt == ZshOpt::NoMatch;
        match (on(opt), default) {
            (true, false) => s.push_str(&format!(" +{name}")),
            (false, true) => s.push_str(&format!(" -{name}")),
            _ => {}
        }
    }
    s
}

fn tracker_states(script: &str) -> Vec<(String, String)> {
    let p = parse(script, Shell::Zsh);
    let params = make_params(&p, Shell::Zsh);
    probes(&p.ast, p.root)
        .into_iter()
        .map(|(label, id)| {
            let d = diff(|o| params.zsh_option(id, o));
            (label, d)
        })
        .collect()
}

struct Row {
    name: &'static str,
    script: &'static str,
    /// (label, state) as the tracker must answer.
    expect: &'static [(&'static str, &'static str)],
    /// zsh's own answers where they differ from `expect` (static rules).
    zsh: &'static [(&'static str, &'static str)],
    /// Needs the parser to accept anonymous functions (lane P-A).
    anonymous: bool,
}

const fn row(name: &'static str, script: &'static str, expect: &'static [(&'static str, &'static str)]) -> Row {
    Row { name, script, expect, zsh: &[], anonymous: false }
}

const NM: &str = " -nomatch";
const EG: &str = " +extendedglob";
const NG: &str = " +nullglob";
const LO: &str = " +localoptions";
const SH_BUNDLE: &str = " +shwordsplit +ksharrays +globsubst +bsdecho +shglob -nomatch";
const KSH_BUNDLE: &str = " +shwordsplit +ksharrays +globsubst +kshglob +shglob -nomatch +localoptions";

#[allow(clippy::too_many_lines)]
fn rows() -> Vec<Row> {
    vec![
        // ---- defaults --------------------------------------------------
        // zsh -fc 'probe a' -> "a:" (only nomatch is on by default)
        row("defaults", "probe a", &[("a", "")]),
        // ---- spelling: case, underscores, no prefix ---------------------
        // setopt extendedglob -> a: +extendedglob
        row("setopt_plain", "setopt extendedglob\nprobe a", &[("a", EG)]),
        // setopt ExtendedGlob / EXTENDEDGLOB -> on
        row(
            "setopt_mixed_case",
            "setopt ExtendedGlob\nprobe a\nunsetopt extendedglob\nsetopt EXTENDEDGLOB\nprobe b",
            &[("a", EG), ("b", EG)],
        ),
        // setopt extended_glob, Ext_ended_Glob -> on (underscores anywhere)
        row(
            "setopt_underscores",
            "setopt extended_glob\nprobe a\nunsetopt extendedglob\nsetopt Ext_ended_Glob\nprobe b",
            &[("a", EG), ("b", EG)],
        ),
        // setopt nonomatch -> nomatch off
        row("setopt_no_prefix", "setopt nonomatch\nprobe a", &[("a", NM)]),
        // setopt no_nomatch, NO_NOMATCH -> nomatch off
        row(
            "setopt_no_underscore_prefix",
            "setopt no_nomatch\nprobe a\nsetopt nomatch\nsetopt NO_NOMATCH\nprobe b",
            &[("a", NM), ("b", NM)],
        ),
        // setopt no_no_match -> nomatch off (the name is nonomatch)
        row("setopt_no_no_match", "setopt no_no_match\nprobe a", &[("a", NM)]),
        // setopt nomatch -> still on (the option itself starts with "no")
        row("setopt_nomatch_is_the_option", "setopt nomatch\nprobe a", &[("a", "")]),
        // setopt -o NOMATCH -> on
        row("setopt_dash_o_nomatch_is_the_option", "setopt -o NOMATCH\nprobe a", &[("a", "")]),
        // setopt extendedglob nullglob -> both
        row("setopt_several", "setopt extendedglob nullglob\nprobe a", &[("a", " +extendedglob +nullglob")]),
        // setopt extendedglob nonullglob -> extendedglob only
        row("setopt_several_with_no", "setopt extendedglob nonullglob\nprobe a", &[("a", EG)]),
        // setopt promptvars -> promptsubst on (zsh's alias)
        row("setopt_alias_promptvars", "setopt promptvars\nprobe a", &[("a", " +promptsubst")]),
        // setopt bogusopt extendedglob -> error on stderr, extendedglob on
        row("setopt_unknown_option_ignored", "setopt bogusopt extendedglob\nprobe a", &[("a", EG)]),
        // setopt extendedglob; unsetopt extendedglob -> off
        row("unsetopt_plain", "setopt extendedglob\nunsetopt extendedglob\nprobe a", &[("a", "")]),
        // unsetopt nomatch -> off
        row("unsetopt_nomatch", "unsetopt nomatch\nprobe a", &[("a", NM)]),
        // unsetopt noextendedglob -> extendedglob ON
        row("unsetopt_no_prefix_sets", "unsetopt noextendedglob\nprobe a", &[("a", EG)]),
        // unsetopt NO_NOMATCH -> nomatch on (already)
        row("unsetopt_no_nomatch_keeps_on", "unsetopt NO_NOMATCH\nprobe a", &[("a", "")]),
        // setopt extendedglob nullglob; unsetopt extendedglob nullglob nomatch
        row(
            "unsetopt_several",
            "setopt extendedglob nullglob\nunsetopt extendedglob nullglob nomatch\nprobe a",
            &[("a", NM)],
        ),
        // ---- setopt -o / +o, set -o / +o -------------------------------
        // setopt -o extendedglob -> on
        row("setopt_dash_o", "setopt -o extendedglob\nprobe a", &[("a", EG)]),
        // setopt -o extendedglob nullglob -> both on (only the next name is the -o's)
        row(
            "setopt_dash_o_then_plain",
            "setopt -o extendedglob nullglob\nprobe a",
            &[("a", " +extendedglob +nullglob")],
        ),
        // setopt extendedglob; setopt +o extendedglob nullglob -> extendedglob off, nullglob on
        row(
            "setopt_plus_o_inverts_next_only",
            "setopt extendedglob\nsetopt +o extendedglob nullglob\nprobe a",
            &[("a", NG)],
        ),
        // setopt +o extendedglob +o nullglob -> both stay off
        row("setopt_plus_o_twice", "setopt +o extendedglob +o nullglob\nprobe a", &[("a", "")]),
        // setopt extendedglob nullglob; unsetopt -o extendedglob nullglob -> both off
        row(
            "unsetopt_dash_o",
            "setopt extendedglob nullglob\nunsetopt -o extendedglob nullglob\nprobe a",
            &[("a", "")],
        ),
        // unsetopt +o extendedglob -> extendedglob ON
        row("unsetopt_plus_o_sets", "unsetopt +o extendedglob\nprobe a", &[("a", EG)]),
        // setopt -- extendedglob -> on
        row("setopt_double_dash", "setopt -- extendedglob\nprobe a", &[("a", EG)]),
        // DIVERGENCE: `setopt -m 'extended*'` takes patterns; the tracker does not expand them.
        // zsh -fc "setopt -m 'extended*'; probe a" -> +extendedglob
        Row {
            name: "setopt_dash_m_patterns_not_followed",
            script: "setopt -m 'extended*'\nprobe a",
            expect: &[("a", "")],
            zsh: &[("a", EG)],
            anonymous: false,
        },
        // set -o extendedglob; set +o nomatch -> +extendedglob -nomatch
        row("set_o_and_plus_o", "set -o extendedglob\nset +o nomatch\nprobe a", &[("a", " +extendedglob -nomatch")]),
        // set -o nonomatch -> nomatch off
        row("set_o_no_prefix", "set -o nonomatch\nprobe a", &[("a", NM)]),
        // set -o noextendedglob -> off
        row("set_o_no_prefix_off", "set -o extendedglob\nset -o noextendedglob\nprobe a", &[("a", "")]),
        // set +o nopipefail -> pipefail ON
        row("set_plus_o_no_prefix_sets", "set +o nopipefail\nprobe a", &[("a", " +pipefail")]),
        // set -o NO_pipefail -> off
        row("set_o_no_underscore_prefix", "set -o pipefail\nset -o NO_pipefail\nprobe a", &[("a", "")]),
        // set -o pipefail -o errreturn -> both
        row("set_two_dash_o", "set -o pipefail -o errreturn\nprobe a", &[("a", " +errreturn +pipefail")]),
        // set -eo pipefail -> pipefail on (flag cluster)
        row("set_flag_cluster", "set -eo pipefail\nprobe a", &[("a", " +pipefail")]),
        // set -o pipefail extendedglob -> only pipefail (the rest are positional)
        row("set_stops_at_positional", "set -o pipefail extendedglob\nprobe a", &[("a", " +pipefail")]),
        // set -o nullglob foo -o extendedglob -> only nullglob
        row("set_stops_at_positional_2", "set -o nullglob foo -o extendedglob\nprobe a", &[("a", NG)]),
        // set -o ksharrays; set +o ksharrays -> off
        row("set_plus_o_off", "set -o ksharrays\nset +o ksharrays\nprobe a", &[("a", "")]),
        // set -y -> shwordsplit on; set +y off
        row("set_dash_y", "set -y\nprobe a\nset +y\nprobe b", &[("a", " +shwordsplit"), ("b", "")]),
        // set -3 -> nomatch off; set +3 -> on
        row("set_dash_3_is_nonomatch", "set -3\nprobe a\nset +3\nprobe b", &[("a", NM), ("b", "")]),
        // set -A arr x; set -o nullglob -> nullglob on (the array form ends this set)
        row("set_array_then_set_o", "set -A arr x\nset -o nullglob\nprobe a", &[("a", NG)]),
        // builtin setopt nullglob; noglob setopt nullglob -> on
        row("builtin_prefix", "builtin setopt nullglob\nprobe a", &[("a", NG)]),
        row("noglob_prefix", "noglob setopt nullglob\nprobe a", &[("a", NG)]),
        // command setopt nullglob -> "command not found: setopt", nothing changes
        row("command_prefix_is_not_the_builtin", "command setopt nullglob\nprobe a", &[("a", "")]),
        // ---- options[x]=on|off ------------------------------------------
        // options[extendedglob]=on -> on
        row("options_assign_on", "options[extendedglob]=on\nprobe a", &[("a", EG)]),
        // options[extendedglob]=off; options[nullglob]=on
        row(
            "options_assign_off_and_on",
            "options[extendedglob]=on\noptions[extendedglob]=off\noptions[nullglob]=on\nprobe a",
            &[("a", NG)],
        ),
        // options[ExtendedGlob]=on, options[extended_glob]=on -> on
        row(
            "options_assign_spelling",
            "options[ExtendedGlob]=on\nprobe a\noptions[extendedglob]=off\noptions[extended_glob]=on\nprobe b",
            &[("a", EG), ("b", EG)],
        ),
        // The `no` prefix is honoured, negated (controller correction, zsh 5.9):
        // zsh -fc 'options[noextendedglob]=off; echo $options[extendedglob]; options[noextendedglob]=on; echo $options[extendedglob]'
        // prints on, then off. options[nonomatch]=on makes $options[nomatch] off.
        row("options_assign_no_prefix_off_sets", "options[noextendedglob]=off\nprobe a", &[("a", EG)]),
        row(
            "options_assign_no_prefix_on_clears",
            "options[extendedglob]=on\noptions[noextendedglob]=on\nprobe a",
            &[("a", "")],
        ),
        row("options_assign_nonomatch_on", "options[nonomatch]=on\nprobe a", &[("a", NM)]),
        row("options_assign_no_underscore_prefix", "options[NO_nullglob]=off\nprobe a", &[("a", NG)]),
        // options[nomatch]=off -> nomatch off
        row("options_assign_nomatch_off", "options[nomatch]=off\nprobe a", &[("a", NM)]),
        // options[extendedglob]=1 / yes / true / 0 / ON -> "invalid value", unchanged
        row(
            "options_assign_bad_values_ignored",
            "options[extendedglob]=1\noptions[extendedglob]=yes\noptions[extendedglob]=true\noptions[extendedglob]=ON\nprobe a",
            &[("a", "")],
        ),
        // options[extendedglob]=on options[nullglob]=on (one command) -> both
        row(
            "options_assign_two_in_one_command",
            "options[extendedglob]=on options[nullglob]=on\nprobe a",
            &[("a", " +extendedglob +nullglob")],
        ),
        // options[bogus]=on -> ignored
        row("options_assign_unknown_ignored", "options[bogus]=on\nprobe a", &[("a", "")]),
        // ---- emulate ------------------------------------------------------
        // setopt <all 14 on, nomatch off>; emulate zsh -> all reset except promptsubst
        row(
            "emulate_zsh_resets_all_but_promptsubst",
            "setopt shwordsplit ksharrays globsubst rcexpandparam bsdecho promptsubst extendedglob kshglob shglob nullglob nonomatch localoptions errreturn pipefail\nemulate zsh\nprobe a",
            &[("a", " +promptsubst")],
        ),
        // the same with -R: promptsubst is reset too
        row(
            "emulate_R_zsh_resets_everything",
            "setopt shwordsplit ksharrays globsubst rcexpandparam bsdecho promptsubst extendedglob kshglob shglob nullglob nonomatch localoptions errreturn pipefail\nemulate -R zsh\nprobe a",
            &[("a", "")],
        ),
        // emulate zsh after setopt kshglob -> kshglob off
        row("emulate_zsh_resets_kshglob", "setopt kshglob\nemulate zsh\nprobe a", &[("a", "")]),
        // emulate sh -> shwordsplit ksharrays globsubst bsdecho shglob on, nomatch off
        row("emulate_sh_bundle", "emulate sh\nprobe a", &[("a", SH_BUNDLE)]),
        // emulate ksh -> sh bundle, but bsdecho off and kshglob, localoptions on
        row("emulate_ksh_bundle", "emulate ksh\nprobe a", &[("a", KSH_BUNDLE)]),
        // emulate bash -> same as emulate sh in zsh 5.9
        row("emulate_bash_is_sh", "emulate bash\nprobe a", &[("a", SH_BUNDLE)]),
        // emulate sh with every option set first: bundle values win, promptsubst untouched
        row(
            "emulate_sh_resets_the_rest",
            "setopt extendedglob kshglob nullglob errreturn pipefail promptsubst\nemulate sh\nprobe a",
            &[("a", " +shwordsplit +ksharrays +globsubst +bsdecho +promptsubst +shglob -nomatch")],
        ),
        // emulate -L sh -> sh bundle + localoptions
        row(
            "emulate_L_sh",
            "emulate -L sh\nprobe a",
            &[("a", " +shwordsplit +ksharrays +globsubst +bsdecho +shglob -nomatch +localoptions")],
        ),
        // emulate -R sh -> sh bundle + promptsubst on
        row(
            "emulate_R_sh",
            "emulate -R sh\nprobe a",
            &[("a", " +shwordsplit +ksharrays +globsubst +bsdecho +promptsubst +shglob -nomatch")],
        ),
        // emulate -LR ksh -> ksh bundle + promptsubst
        row(
            "emulate_LR_ksh",
            "emulate -LR ksh\nprobe a",
            &[("a", " +shwordsplit +ksharrays +globsubst +promptsubst +kshglob +shglob -nomatch +localoptions")],
        ),
        // emulate -R ksh -> ksh bundle + promptsubst
        row(
            "emulate_R_ksh",
            "emulate -R ksh\nprobe a",
            &[("a", " +shwordsplit +ksharrays +globsubst +promptsubst +kshglob +shglob -nomatch +localoptions")],
        ),
        // emulate -L zsh -> localoptions only
        row("emulate_L_zsh", "emulate -L zsh\nprobe a", &[("a", LO)]),
        // emulate -R zsh -> defaults (no localoptions)
        row("emulate_R_zsh", "emulate -R zsh\nprobe a", &[("a", "")]),
        // emulate -L zsh -o extended_glob -> extendedglob + localoptions
        row(
            "emulate_L_zsh_o_extended_glob",
            "emulate -L zsh -o extended_glob\nprobe a",
            &[("a", " +extendedglob +localoptions")],
        ),
        // emulate -L zsh -o nullglob -o extended_glob
        row(
            "emulate_two_o",
            "emulate -L zsh -o nullglob -o extended_glob\nprobe a",
            &[("a", " +extendedglob +nullglob +localoptions")],
        ),
        // emulate zsh -o extended_glob -o no_nomatch
        row(
            "emulate_o_no_prefix",
            "emulate zsh -o extended_glob -o no_nomatch\nprobe a",
            &[("a", " +extendedglob -nomatch")],
        ),
        // emulate sh -o noshwordsplit
        row(
            "emulate_sh_o_noshwordsplit",
            "emulate sh\nemulate sh -o noshwordsplit\nprobe a",
            &[("a", " +ksharrays +globsubst +bsdecho +shglob -nomatch")],
        ),
        // emulate -L zsh -o nolocaloptions -> localoptions stays ON (-L is applied after -o)
        row("emulate_L_wins_over_o_nolocaloptions", "emulate -L zsh -o nolocaloptions\nprobe a", &[("a", LO)]),
        // emulate -L zsh -L -> fine
        row("emulate_L_twice", "emulate -L zsh -L\nprobe a", &[("a", LO)]),
        // emulate zsh -c "setopt nullglob" -> runs under emulation then restores: no change
        row("emulate_c_changes_nothing", "setopt extendedglob\nemulate sh -c 'setopt nullglob'\nprobe a", &[("a", EG)]),
        // emulate -o nullglob zsh -> "bad option: -o", nothing changes
        row("emulate_o_before_shell_is_an_error", "emulate -o nullglob zsh\nprobe a", &[("a", "")]),
        // emulate zsh -o nullglob extended_glob -> "unknown argument", nothing changes
        row(
            "emulate_stray_argument_is_an_error",
            "setopt kshglob\nemulate zsh -o nullglob extended_glob\nprobe a",
            &[("a", " +kshglob")],
        ),
        // emulate -L (no shell) -> "not enough arguments"
        row("emulate_without_shell", "emulate -L\nprobe a", &[("a", "")]),
        // emulate -L sh; emulate -L zsh -> back to the zsh defaults
        row("emulate_sh_then_zsh", "emulate sh\nemulate -L zsh\nprobe a", &[("a", LO)]),
        // ---- scope: functions -------------------------------------------
        // f(){ setopt extendedglob; }; f -> leaks
        row("function_leaks_without_localoptions", "f() { setopt extendedglob; }\nf\nprobe a", &[("a", EG)]),
        // the leak starts at the call, not at the definition
        row(
            "function_leak_starts_at_the_call",
            "f() { setopt extendedglob; }\nprobe a\nf\nprobe b",
            &[("a", ""), ("b", EG)],
        ),
        // f(){ setopt localoptions; setopt extendedglob; probe in }; f -> in: both, out: restored
        row(
            "function_localoptions_first",
            "f() { setopt localoptions; setopt extendedglob; probe in; }\nf\nprobe out",
            &[("in", " +extendedglob +localoptions"), ("out", "")],
        ),
        // setopt localoptions AFTER another setopt still restores everything
        row(
            "function_localoptions_last",
            "f() { setopt extendedglob; setopt localoptions; }\nf\nprobe out",
            &[("out", "")],
        ),
        // a later nolocaloptions cancels it -> leaks
        row(
            "function_nolocaloptions_cancels",
            "f() { setopt localoptions extendedglob; setopt nolocaloptions; }\nf\nprobe out",
            &[("out", EG)],
        ),
        // unsetopt localoptions cancels it too
        row(
            "function_unsetopt_localoptions_cancels",
            "f() { setopt localoptions; unsetopt localoptions; setopt nullglob; }\nf\nprobe out",
            &[("out", NG)],
        ),
        // off, on again, restored
        row(
            "function_localoptions_reenabled",
            "f() { setopt localoptions; setopt nullglob; setopt nolocaloptions; setopt localoptions; }\nf\nprobe out",
            &[("out", "")],
        ),
        // setopt/unsetopt cancel each other inside a restoring function, nothing leaks
        row(
            "function_localoptions_restores_unset",
            "f() { setopt localoptions; setopt nullglob; unsetopt nullglob; setopt extendedglob; }\nf\nprobe out",
            &[("out", "")],
        ),
        // if / { } / while inside a function do not scope
        row(
            "function_blocks_do_not_scope",
            "f() { if true; then setopt extendedglob; fi; { setopt kshglob; }; while false; do :; done; }\nf\nprobe out",
            &[("out", " +extendedglob +kshglob")],
        ),
        // setopt in an if in a function with localoptions: visible after the if, gone after the call
        row(
            "function_localoptions_setopt_in_if",
            "f() { setopt localoptions; if true; then setopt extendedglob; fi; probe in; }\nf\nprobe out",
            &[("in", " +extendedglob +localoptions"), ("out", "")],
        ),
        // nolocaloptions inside an if still cancels
        row(
            "function_nolocaloptions_in_if",
            "f() { setopt localoptions; setopt nullglob; if true; then setopt nolocaloptions; fi; }\nf\nprobe out",
            &[("out", NG)],
        ),
        // if / while / { } at top level do not scope
        row("toplevel_if_does_not_scope", "if true; then setopt extendedglob; fi\nprobe a", &[("a", EG)]),
        row("toplevel_while_does_not_scope", "while setopt nullglob; do break; done\nprobe a", &[("a", NG)]),
        row("toplevel_brace_group_does_not_scope", "{ setopt nullglob; }\nprobe a", &[("a", NG)]),
        row("toplevel_for_does_not_scope", "for x in 1 2; do setopt nullglob; done\nprobe a", &[("a", NG)]),
        row("toplevel_case_does_not_scope", "case x in x) setopt nullglob ;; esac\nprobe a", &[("a", NG)]),
        // emulate -L zsh in a function: restored on return
        row(
            "function_emulate_L",
            "f() { emulate -L zsh; setopt extendedglob; probe in; }\nf\nprobe out",
            &[("in", " +extendedglob +localoptions"), ("out", "")],
        ),
        // emulate zsh (no -L) in a function leaks the reset
        row(
            "function_emulate_without_L_leaks",
            "setopt kshglob\nf() { emulate zsh; setopt extendedglob; }\nf\nprobe out",
            &[("out", EG)],
        ),
        // emulate ksh turns localoptions on itself, so it restores
        row("function_emulate_ksh_restores", "f() { emulate ksh; setopt nullglob; }\nf\nprobe out", &[("out", "")]),
        // emulate -L zsh then unsetopt localoptions -> the emulate reset leaks
        row(
            "function_emulate_L_then_unset_localoptions",
            "setopt nullglob\nf() { emulate -L zsh; unsetopt localoptions; }\nf\nprobe out",
            &[("out", "")],
        ),
        // top-level emulate -L zsh: localoptions on for the rest of the script
        row("toplevel_emulate_L_zsh", "emulate -L zsh\nsetopt nullglob\nprobe a", &[("a", " +nullglob +localoptions")]),
        // ... and so a function called after it restores on return
        row(
            "function_after_toplevel_emulate_L_restores",
            "emulate -L zsh\nf() { setopt extendedglob; }\nf\nprobe out",
            &[("out", LO)],
        ),
        // zsh -fc 'emulate -L zsh; f(){ unsetopt localoptions; setopt nullglob }; f; probe out'
        // -> out: +nullglob +localoptions (localoptions is back to its value at the call)
        row(
            "function_cannot_clear_the_callers_localoptions",
            "emulate -L zsh\nf() { unsetopt localoptions; setopt nullglob; }\nf\nprobe out",
            &[("out", " +nullglob +localoptions")],
        ),
        // zsh -fc 'f(){ unsetopt localoptions }; g(){ setopt localoptions; f; setopt extendedglob }; g; probe out' -> out:
        row(
            "callee_unsetting_localoptions_does_not_cancel_the_callers",
            "f() { unsetopt localoptions; }\ng() { setopt localoptions; f; setopt extendedglob; }\ng\nprobe out",
            &[("out", "")],
        ),
        // zsh -fc 'f(){ setopt localoptions }; g(){ f; setopt extendedglob }; g; probe out' -> out: +extendedglob
        row(
            "callee_setting_localoptions_does_not_restore_for_the_caller",
            "f() { setopt localoptions; }\ng() { f; setopt extendedglob; }\ng\nprobe out",
            &[("out", EG)],
        ),
        // zsh -fc 'f(){ unsetopt localoptions; setopt nullglob }; g(){ setopt localoptions; f; probe in }; g; probe out'
        // -> in: +nullglob +localoptions, out:
        row(
            "leak_into_a_restoring_caller",
            "f() { unsetopt localoptions; setopt nullglob; }\ng() { setopt localoptions; f; probe in; }\ng\nprobe out",
            &[("in", " +nullglob +localoptions"), ("out", "")],
        ),
        // set -o in a function leaks like setopt
        row("function_set_o_leaks", "f() { set -o pipefail; }\nf\nprobe out", &[("out", " +pipefail")]),
        // ---- scope: nested functions and calls ------------------------
        // f leaks, g calls f and leaks it on
        row("nested_call_leaks_through", "f() { setopt nullglob; }\ng() { f; }\ng\nprobe out", &[("out", NG)]),
        // g has localoptions: the leak of f stops there
        row(
            "nested_call_restored_by_caller",
            "f() { setopt nullglob; }\ng() { setopt localoptions; f; }\ng\nprobe out",
            &[("out", "")],
        ),
        // helper defined below its caller
        row("call_of_helper_defined_later", "f() { g; }\ng() { setopt nullglob; }\nf\nprobe out", &[("out", NG)]),
        // f restores, f2 (defined later) leaks, f calls it: restored by f
        row(
            "restoring_caller_of_later_helper",
            "f() { setopt localoptions; f2; }\nf2() { setopt nullglob; }\nf\nprobe out",
            &[("out", "")],
        ),
        // a function defined inside another function after its setopt (leaks) sees it
        row(
            "inner_definition_sees_outer_setopt",
            "f() { setopt extendedglob; g() { probe in; }; }\nf\ng\nprobe out",
            &[("in", EG), ("out", EG)],
        ),
        // inner function inherits localoptions from the outer one: g restores too
        row(
            "inner_function_inherits_localoptions",
            "f() { setopt localoptions kshglob; g() { setopt nullglob; probe gin; }; g; probe fin; }\nf\nprobe out",
            &[("gin", " +kshglob +nullglob +localoptions"), ("fin", " +kshglob +localoptions"), ("out", "")],
        ),
        // a function that calls itself
        row("recursive_function", "f() { setopt nullglob; [[ -n $x ]] && f; }\nf\nprobe out", &[("out", NG)]),
        // the call forms that run the function in the current shell
        row("call_with_assignment_prefix", "f() { setopt nullglob; }\nx=1 f\nprobe out", &[("out", NG)]),
        row("call_in_if_condition", "f() { setopt nullglob; }\nif f; then :; fi\nprobe out", &[("out", NG)]),
        row("call_in_for_body", "f() { setopt nullglob; }\nfor x in 1; do f; done\nprobe out", &[("out", NG)]),
        row("call_last_in_pipeline", "f() { setopt nullglob; }\ncat </dev/null | f\nprobe out", &[("out", NG)]),
        // and the ones that fork
        row("call_in_subshell", "f() { setopt nullglob; }\n(f)\nprobe out", &[("out", "")]),
        row("call_in_command_substitution", "f() { setopt nullglob; }\ny=$(f)\nprobe out", &[("out", "")]),
        row("call_in_background", "f() { setopt nullglob; }\nf &\nprobe out", &[("out", "")]),
        row("call_first_in_pipeline", "f() { setopt nullglob; }\nf | cat\nprobe out", &[("out", "")]),
        // command f does not run a function
        row("command_f_is_not_the_function", "f() { setopt nullglob; }\ncommand f\nprobe out", &[("out", "")]),
        // function keyword, and one with the options set at top level first
        row(
            "function_keyword_form",
            "function f { setopt localoptions nullglob; probe in; }\nf\nprobe out",
            &[("in", " +nullglob +localoptions"), ("out", "")],
        ),
        // ---- scope: forked shells ----------------------------------------
        row("subshell_does_not_leak", "( setopt extendedglob; probe in )\nprobe out", &[("in", EG), ("out", "")]),
        row("command_substitution_does_not_leak", "x=$(setopt extendedglob)\nprobe out", &[("out", "")]),
        row("backticks_do_not_leak", "x=`setopt extendedglob`\nprobe out", &[("out", "")]),
        row("first_pipeline_element_does_not_leak", "setopt extendedglob | cat\nprobe out", &[("out", "")]),
        row("last_pipeline_element_leaks", "cat </dev/null | setopt extendedglob\nprobe out", &[("out", EG)]),
        row("background_does_not_leak", "setopt extendedglob &\nprobe out", &[("out", "")]),
        // ---- static rule: the definition site ----------------------------
        // setopt extendedglob; f(){ setopt nullglob; probe in }; f; probe out
        row(
            "body_setopt_adjusts_the_definition_state",
            "setopt extendedglob\nf() { setopt nullglob; probe in; }\nf\nprobe out",
            &[("in", " +extendedglob +nullglob"), ("out", " +extendedglob +nullglob")],
        ),
        // DIVERGENCE (static rule): option changed after the definition and before the call.
        // zsh -fc 'setopt nullglob; f(){ probe in; }; unsetopt nullglob; f' -> "in:" (the caller's state)
        // The tracker answers the definition-site state.
        Row {
            name: "option_changed_between_definition_and_call_static_rule",
            script: "setopt nullglob\nf() { probe in; }\nunsetopt nullglob\nf\nprobe out",
            expect: &[("in", NG), ("out", "")],
            zsh: &[("in", ""), ("out", "")],
            anonymous: false,
        },
        // DIVERGENCE (static rule): the same through a localoptions function.
        // zsh -fc 'setopt extendedglob; f(){ setopt localoptions; probe in; }; unsetopt extendedglob; f' -> in: +localoptions
        Row {
            name: "option_changed_between_definition_and_call_localoptions",
            script: "setopt extendedglob\nf() { setopt localoptions; probe in; }\nunsetopt extendedglob\nf\nprobe out",
            expect: &[("in", " +extendedglob +localoptions"), ("out", "")],
            zsh: &[("in", LO), ("out", "")],
            anonymous: false,
        },
        // DIVERGENCE (branches are walked as if they all ran): false && f is not executed by zsh.
        // zsh -fc 'f(){ setopt nullglob; }; false && f; probe out' -> "out:"
        Row {
            name: "short_circuited_call_is_applied_static_rule",
            script: "f() { setopt nullglob; }\nfalse && f\nprobe out",
            expect: &[("out", NG)],
            zsh: &[("out", "")],
            anonymous: false,
        },
        // ---- combinations -------------------------------------------------
        row(
            "combo_emulate_L_extended_glob_in_function_with_if_and_call",
            "f() {\n  emulate -L zsh -o extended_glob\n  if true; then setopt nullglob; fi\n  probe in\n}\nf\nprobe out",
            &[("in", " +extendedglob +nullglob +localoptions"), ("out", "")],
        ),
        row(
            "combo_outer_setopt_inner_unsetopt_in_localoptions_function",
            "setopt extendedglob nullglob\nf() { setopt localoptions; unsetopt extendedglob; probe in; }\nf\nprobe out",
            &[("in", " +nullglob +localoptions"), ("out", " +extendedglob +nullglob")],
        ),
        row(
            "combo_function_defined_in_subshell_is_still_known",
            "( f() { setopt nullglob; }; f; probe in )\nprobe out",
            &[("in", NG), ("out", "")],
        ),
        row(
            "combo_two_functions_one_leaks_one_restores",
            "a() { setopt localoptions; setopt nullglob; }\nb() { setopt extendedglob; }\na\nprobe one\nb\nprobe two",
            &[("one", ""), ("two", EG)],
        ),
        row(
            "combo_redefined_function",
            "f() { setopt nullglob; }\nf() { setopt localoptions; setopt extendedglob; }\nf\nprobe out",
            &[("out", "")],
        ),
        // ---- anonymous functions (parser lane P-A) -------------------------
        // zsh -fc '() { setopt nullglob }; probe out' -> +nullglob
        Row {
            anonymous: true,
            ..row("anon_leaks_without_localoptions", "() { setopt nullglob; }\nprobe out", &[("out", NG)])
        },
        // zsh -fc '() { setopt localoptions nullglob }; probe out' -> ""
        Row {
            anonymous: true,
            ..row(
                "anon_localoptions_restores",
                "() { setopt localoptions nullglob; probe in; }\nprobe out",
                &[("in", " +nullglob +localoptions"), ("out", "")],
            )
        },
        // zsh -fc '() { emulate -L zsh; setopt nullglob }; probe out' -> ""
        Row {
            anonymous: true,
            ..row(
                "anon_emulate_L_restores",
                "() { emulate -L zsh; setopt nullglob; probe in; }\nprobe out",
                &[("in", " +nullglob +localoptions"), ("out", "")],
            )
        },
        // a nolocaloptions inside the anonymous function cancels it
        Row {
            anonymous: true,
            ..row(
                "anon_nolocaloptions_cancels",
                "() { setopt localoptions nullglob; setopt nolocaloptions; }\nprobe out",
                &[("out", NG)],
            )
        },
        // with arguments, and as the idiom `() { ... } &|`... the body still runs here
        Row {
            anonymous: true,
            ..row(
                "anon_with_arguments",
                "() { setopt localoptions nullglob; probe in; } a b\nprobe out",
                &[("in", " +nullglob +localoptions"), ("out", "")],
            )
        },
        // inside a named function with localoptions: the anonymous function inherits it and restores too
        // zsh -fc 'f(){ setopt localoptions; () { setopt nullglob }; probe in }; f; probe out' -> in: +localoptions
        Row {
            anonymous: true,
            ..row(
                "anon_inside_restoring_function",
                "f() { setopt localoptions; () { setopt nullglob; }; probe in; }\nf\nprobe out",
                &[("in", LO), ("out", "")],
            )
        },
    ]
}

fn expect_map(r: &Row, zsh: bool) -> BTreeMap<String, String> {
    let mut m: BTreeMap<String, String> = r.expect.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    if zsh {
        for (k, v) in r.zsh {
            m.insert(k.to_string(), v.to_string());
        }
    }
    m
}

#[test]
fn rows_have_unique_names_and_labels() {
    let rows = rows();
    let mut names: Vec<_> = rows.iter().map(|r| r.name).collect();
    names.sort_unstable();
    let n = names.len();
    names.dedup();
    assert_eq!(names.len(), n, "duplicate row name");
    for r in &rows {
        let mut labels: Vec<_> = r.expect.iter().map(|(l, _)| *l).collect();
        labels.sort_unstable();
        let n = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), n, "row {}: duplicate label", r.name);
        assert!(!r.expect.is_empty(), "row {}: no expectation", r.name);
    }
}

/// The table, run through the tracker.
#[test]
fn tracker_matches_the_table() {
    let mut failures = Vec::new();
    for r in rows().iter().filter(|r| !r.anonymous) {
        let got: Vec<(String, String)> = tracker_states(r.script);
        let want: Vec<(String, String)> = r.expect.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let mut got_sorted = got.clone();
        got_sorted.sort();
        let mut want_sorted = want.clone();
        want_sorted.sort();
        if got_sorted != want_sorted {
            failures.push(format!("row {}:\n  script {:?}\n  want {:?}\n  got  {:?}", r.name, r.script, want, got));
        }
    }
    assert!(failures.is_empty(), "{} row(s) differ:\n{}", failures.len(), failures.join("\n"));
}

/// Every anonymous-function row, once lane P-A lets them parse.
#[test]
fn tracker_matches_the_table_anonymous_functions() {
    let mut failures = Vec::new();
    for r in rows().iter().filter(|r| r.anonymous) {
        let mut got = tracker_states(r.script);
        got.sort();
        let mut want: Vec<(String, String)> = r.expect.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        want.sort();
        if got != want {
            failures.push(format!("row {}:\n  want {:?}\n  got  {:?}", r.name, want, got));
        }
    }
    assert!(failures.is_empty(), "{} row(s) differ:\n{}", failures.len(), failures.join("\n"));
}

const PRELUDE: &str = r#"probe() {
  local o s=""
  for o in shwordsplit ksharrays globsubst rcexpandparam bsdecho promptsubst extendedglob kshglob shglob nullglob nomatch localoptions errreturn pipefail; do
    if [[ -o $o ]]; then
      [[ $o == nomatch ]] || s="$s +$o"
    else
      [[ $o == nomatch ]] && s="$s -$o"
    fi
  done
  print -r -- "$1:$s"
}
"#;

fn run_zsh(script: &str) -> Option<(String, String)> {
    let zsh = std::path::Path::new("/usr/bin/zsh");
    if !zsh.exists() {
        return None;
    }
    let out = std::process::Command::new(zsh)
        .arg("-fc")
        .arg(format!("{PRELUDE}{script}"))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run zsh");
    Some((String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned()))
}

/// Every row, run under real zsh 5.9: what zsh prints at each probe must be
/// the table's value (or the row's recorded `zsh:` answer where the tracker
/// is static on purpose). Skipped, loudly, when there is no zsh.
#[test]
fn rows_match_real_zsh() {
    if !std::path::Path::new("/usr/bin/zsh").exists() {
        // Not a failure: CI images without zsh still run the rest of the table.
        eprintln!("SKIPPED rows_match_real_zsh: /usr/bin/zsh is missing, so the zsh option rows were NOT checked");
        return;
    }
    let mut ran = 0;
    let mut failures = Vec::new();
    for r in rows() {
        let (stdout, _stderr) = run_zsh(r.script).expect("/usr/bin/zsh exists");
        ran += 1;
        let got: BTreeMap<String, String> =
            stdout.lines().filter_map(|l| l.split_once(':')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        if got != expect_map(&r, true) {
            failures.push(format!(
                "row {}:\n  script {:?}\n  zsh printed {:?}\n  table says  {:?}",
                r.name,
                r.script,
                got,
                expect_map(&r, true)
            ));
        }
    }
    assert!(ran > 0);
    assert!(failures.is_empty(), "{} row(s) disagree with zsh:\n{}", failures.len(), failures.join("\n"));
}

// ---- other shells --------------------------------------------------------

/// For every shell but zsh the tracker is never built and `zsh_option` is
/// the foundation's fixed answer.
#[test]
fn non_zsh_shells_never_build_the_tracker_and_keep_the_foundation_values() {
    use ZshOpt::*;
    let script = "setopt extendedglob\nunsetopt nomatch\nemulate zsh\nf() { setopt localoptions; }\nprobe a";
    for shell in [Shell::Sh, Shell::Bash, Shell::Dash, Shell::Ksh, Shell::BusyboxSh] {
        let p = parse(script, shell);
        let params = make_params(&p, shell);
        assert!(params.zsh_options.is_none(), "{shell:?} built a zsh option map");
        for (_, probe_id) in probes(&p.ast, p.root).into_iter().chain([("root".to_string(), p.root)]) {
            for (name, opt) in ALL {
                let want = match opt {
                    ShWordSplit | KshArrays | GlobSubst | BsdEcho | ShGlob => true,
                    KshGlob => matches!(shell, Shell::Ksh | Shell::Bash),
                    PipeFail => params.has_pipefail,
                    RcExpandParam | PromptSubst | ExtendedGlob | NullGlob | NoMatch | LocalOptions | ErrReturn => false,
                };
                assert_eq!(params.zsh_option(probe_id, opt), want, "{shell:?} {name}");
            }
        }
    }
}

#[test]
fn non_zsh_pipefail_follows_set_o_pipefail() {
    for shell in [Shell::Sh, Shell::Bash, Shell::Dash, Shell::Ksh, Shell::BusyboxSh] {
        let p = parse("set -o pipefail\nprobe a", shell);
        let params = make_params(&p, shell);
        let (_, id) = probes(&p.ast, p.root).remove(0);
        assert!(params.zsh_option(id, ZshOpt::PipeFail), "{shell:?}");
        let p = parse("probe a", shell);
        let params = make_params(&p, shell);
        let (_, id) = probes(&p.ast, p.root).remove(0);
        assert!(!params.zsh_option(id, ZshOpt::PipeFail), "{shell:?}");
    }
}

#[test]
fn zsh_builds_the_tracker() {
    let p = parse("probe a", Shell::Zsh);
    assert!(make_params(&p, Shell::Zsh).zsh_options.is_some());
}

// ---- robustness ----------------------------------------------------------

#[test]
fn every_node_of_a_function_body_gets_the_definition_site_state() {
    let p = parse(
        "setopt nullglob\nf() { echo \"$(echo hi)\" | cat; probe in; }\nunsetopt nullglob\nprobe out",
        Shell::Zsh,
    );
    let params = make_params(&p, Shell::Zsh);
    let mut inside = 0;
    p.ast.analyze(
        p.root,
        &mut |id| {
            if matches!(p.ast.get(id), Token::T_Literal(s) if s == "hi") {
                assert!(params.zsh_option(id, ZshOpt::NullGlob), "nullglob should be on inside the body");
                inside += 1;
            }
        },
        &mut |_| {},
    );
    assert_eq!(inside, 1);
}

#[test]
fn an_id_from_another_tree_reads_as_the_defaults() {
    let p = parse("setopt extendedglob\nprobe a", Shell::Zsh);
    let params = make_params(&p, Shell::Zsh);
    let far = Id(p.ast.len() + 1000);
    assert!(!params.zsh_option(far, ZshOpt::ExtendedGlob));
    assert!(params.zsh_option(far, ZshOpt::NoMatch));
}

#[test]
fn the_setopt_command_itself_sees_the_state_before_it() {
    let p = parse("setopt extendedglob\nsetopt nullglob", Shell::Zsh);
    let params = make_params(&p, Shell::Zsh);
    let mut seen = Vec::new();
    p.ast.analyze(
        p.root,
        &mut |id| {
            if let Token::T_SimpleCommand(_, words) = p.ast.get(id) {
                seen.push((
                    get_literal_string(&p.ast, words[1]).unwrap(),
                    params.zsh_option(id, ZshOpt::ExtendedGlob),
                    params.zsh_option(id, ZshOpt::NullGlob),
                ));
            }
        },
        &mut |_| {},
    );
    assert_eq!(seen, vec![("extendedglob".into(), false, false), ("nullglob".into(), true, false)]);
}

#[test]
fn deeply_nested_blocks_do_not_overflow_and_keep_the_state() {
    // 400 nested brace groups: past the tracker's depth cap, so the inner part
    // is labelled with the state reached at the cap.
    let depth = 400;
    let mut script = String::from("setopt nullglob\n");
    script.push_str(&"{ ".repeat(depth));
    script.push_str("probe deep;");
    script.push_str(&" }".repeat(depth));
    script.push_str("\nprobe out");
    let handle =
        std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || tracker_states(&script)).unwrap();
    let got = handle.join().unwrap();
    assert_eq!(got, vec![("deep".to_string(), NG.to_string()), ("out".to_string(), NG.to_string())]);
}

#[test]
fn non_literal_arguments_are_left_alone() {
    // setopt $x and set -- $@ change nothing the tracker can name.
    let got = tracker_states("setopt $x \"$y\"\nset -o $z\nemulate $w\nprobe a");
    assert_eq!(got, vec![("a".to_string(), String::new())]);
}

// ---- the anonymous-function representation, without the parser ------------

/// The parser (lane P-A) builds `() { body }` as
/// `T_BraceGroup([T_Redirecting(T_Function("zsh-anonymous-function", body)),
/// <T_SimpleCommand of that name>])`, a definition plus its immediate call.
/// Until it is merged, a hand-built tree of that shape stands in.
fn hand_built(anon_body_commands: &[(&str, &str)]) -> (Ast, Id, Id) {
    use shelllint_core::ast::Pos;
    use shelllint_core::ast::{FunctionKeyword, FunctionParentheses};
    let pos = Pos { line: 1, column: 1 };
    let mut ast = Ast::new();
    let node = |ast: &mut Ast, t: Token| {
        let id = ast.alloc(pos, pos);
        ast.set(id, t);
        id
    };
    let command = |ast: &mut Ast, words: &[&str]| {
        let ids: Vec<Id> = words
            .iter()
            .map(|w| {
                let lit = node(ast, Token::T_Literal((*w).to_string()));
                node(ast, Token::T_NormalWord(vec![lit]))
            })
            .collect();
        let cmd = node(ast, Token::T_SimpleCommand(vec![], ids));
        node(ast, Token::T_Redirecting(vec![], cmd))
    };
    let body_cmds: Vec<Id> = anon_body_commands.iter().map(|(a, b)| command(&mut ast, &[a, b])).collect();
    let body = node(&mut ast, Token::T_BraceGroup(body_cmds));
    let name = "zsh-anonymous-function".to_string();
    let def = node(&mut ast, Token::T_Function(FunctionKeyword(false), FunctionParentheses(true), name.clone(), body));
    let def = node(&mut ast, Token::T_Redirecting(vec![], def));
    let call = command(&mut ast, &[&name, "arg"]);
    let group = node(&mut ast, Token::T_BraceGroup(vec![def, call]));
    let probe = command(&mut ast, &["probe", "a"]);
    let shebang = node(&mut ast, Token::T_Literal(String::new()));
    let root = node(&mut ast, Token::T_Script(shebang, vec![group, probe]));
    ast.compute_parents(root);
    (ast, root, probe)
}

#[test]
fn an_anonymous_function_leaks_like_a_called_function() {
    let (ast, root, probe) = hand_built(&[("setopt", "nullglob")]);
    let map = shelllint_analyzer::zsh_options::build(&ast, root);
    assert!(map.get(probe, ZshOpt::NullGlob), "an anonymous function without localoptions leaks");
}

#[test]
fn an_anonymous_function_with_localoptions_restores() {
    let (ast, root, probe) = hand_built(&[("setopt", "localoptions"), ("setopt", "nullglob")]);
    let map = shelllint_analyzer::zsh_options::build(&ast, root);
    assert!(!map.get(probe, ZshOpt::NullGlob));
    assert!(!map.get(probe, ZshOpt::LocalOptions));
}

// ---- cheap benchmark -----------------------------------------------------

/// `ZSH_OT_BENCH_FILE=a.zsh:b.zsh cargo test -p shelllint-analyzer --test zsh_options -- --ignored --nocapture bench`
/// prints, for each file that parses, the time of the whole analysis set-up
/// and the part of it that is the option tracker.
#[test]
#[ignore = "benchmark; needs ZSH_OT_BENCH_FILE"]
fn bench_tracker_cost() {
    let mut inputs: Vec<(String, String)> = Vec::new();
    // A function-heavy synthetic file: 5000 functions with options, ifs and calls.
    let synthetic: String = (0..5000)
        .map(|i| {
            format!(
                "f{i}() {{\n  emulate -L zsh -o extended_glob\n  if [[ -n $1 ]]; then setopt nullglob; fi\n  print -r -- ${{1:-x}} | cat\n  g{i}\n}}\ng{i}() {{ setopt localoptions; unsetopt nomatch; }}\nf{i}\n"
            )
        })
        .collect();
    inputs.push(("synthetic".to_string(), synthetic));
    for path in std::env::var("ZSH_OT_BENCH_FILE").unwrap_or_default().split(':').filter(|p| !p.is_empty()) {
        inputs.push((path.to_string(), std::fs::read_to_string(path).expect("read bench file")));
    }
    for (path, script) in inputs {
        let path = path.as_str();
        let t = std::time::Instant::now();
        let mut spec = new_parse_spec();
        spec.script = script.clone();
        spec.shell_type_override = Some(Shell::Zsh);
        let mut pr = shelllint_parser::parse_script(&new_system_interface(), &spec);
        let parse_time = t.elapsed();
        let Some(root) = pr.root else {
            eprintln!("bench {path}: does not parse yet");
            continue;
        };
        pr.ast.compute_parents(root);
        let p = Parsed { ast: pr.ast, root };
        let best = |f: &dyn Fn()| {
            (0..5)
                .map(|_| {
                    let t = std::time::Instant::now();
                    f();
                    t.elapsed()
                })
                .min()
                .unwrap()
        };
        let with = best(&|| drop(make_params(&p, Shell::Zsh)));
        let tracker = best(&|| drop(shelllint_analyzer::zsh_options::build(&p.ast, p.root)));
        eprintln!(
            "bench {path}: {} bytes, {} nodes; parse {parse_time:?}; make_parameters (with tracker) {with:?}; tracker alone {tracker:?}; make_parameters without tracker ~ {:?}",
            script.len(),
            p.ast.len(),
            with.saturating_sub(tracker)
        );
    }
}
