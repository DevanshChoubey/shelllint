// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// Realistic plugin-shaped scripts (our own code, under `scripts/`) with an exact expected
// list of (code, line). Each script passes `zsh -n` (checked below when zsh is installed).
// The lists are derived by reasoning from the design, one justification per entry. A
// script's *absence* of findings matters as much as its findings: every construct in
// them that bash-mode reports wrongly for zsh is listed as "silent because".
//
// Lists are compared as sorted (line, code) pairs, so report order does not matter.
use crate::support::*;

struct Script {
    name: &'static str,
    file_name: &'static str,
    text: &'static str,
    /// (line, code), sorted.
    expect: &'static [(u32, u32)],
}

static SCRIPTS: &[Script] = &[
    Script {
        name: "prompt_theme",
        file_name: "prompt_theme.zsh-theme",
        text: include_str!("scripts/prompt_theme.zsh-theme"),
        // (18, SC2155): `local branch=$(cmd)` masks the status in zsh too (class a).
        // (33, SC2046): an unquoted `$(cmd)` still splits in zsh (class a).
        // Silent on purpose:
        //   line 2 `setopt prompt_subst` makes the single-quoted PROMPT on line 29 (SC2016) fine;
        //   lines 9, 10, 29, 30 assign ZSH_THEME_*, PROMPT, RPROMPT: special parameters, no SC2034 (c);
        //   line 26 `${(j: :)parts}`: flags are valid zsh (SC2296 off, d), no SC2086 without
        //     shwordsplit (b), and it is a use of `parts`, so no SC2034 on line 25;
        //   `%F{..}` and `%(?.a.b)` in single quotes parse without SC1083.
        expect: &[(18, 2155), (33, 2046)],
    },
    Script {
        name: "completion_function",
        file_name: "completion_function.zsh",
        text: include_str!("scripts/completion_function.zsh"),
        // (14, SC2207): `envs=($(cmd))` splits the command output in zsh (class a).
        // Silent on purpose:
        //   line 1 `#compdef` and the zsh shell: no SC2148;
        //   line 8 `profiles=(${(f)"$(cmd)"})`: the zsh idiom; SC2206 needs shwordsplit (b),
        //     SC2296 is off (d), and it is not SC2207;
        //   line 35 `$line[1]`: element 1 in zsh, SC1087 needs ksharrays (b);
        //   `state`/`line` are declared by `local` and read, no SC2154;
        //   `(build|deploy)` / `(clean)` case labels parse, no SC2221/2222 between them;
        //   `$ret` in `return $ret` is not split (b).
        expect: &[(14, 2207)],
    },
    Script {
        name: "hook_plugin",
        file_name: "hook_plugin.zsh",
        text: include_str!("scripts/hook_plugin.zsh"),
        // (10, SC2034): `local started=$EPOCHSECONDS` is never read (class a).
        // Silent on purpose:
        //   `zshexit_functions+=(...)` (line 35): a hook array is special, no SC2034 (c);
        //   functions registered via `add-zsh-hook` (32, 34), `precmd_functions+=` (33), `zle -N` and `*_functions+=` are used;
        //   `{ cmd }` bodies on lines 29/30 (no `;` before `}`): parse, no SC1083;
        //   `%F{yellow}` inside the double-quoted `print -P` string: no SC1083;
        //   EPOCHSECONDS and SLOW_CMD_THRESHOLD are upper case: no SC2154.
        expect: &[(10, 2034)],
    },
    Script {
        name: "emulate_library",
        file_name: "emulate_library.zsh",
        text: include_str!("scripts/emulate_library.zsh"),
        // (21, SC2164): `cd $dir` without `|| return` (class a). It is also unquoted, but
        //   `emulate -L zsh` means no shwordsplit, so no SC2086 on it.
        // Silent on purpose:
        //   `hits=( $dir/*(.Nom[1]) )` (line 12): glob qualifiers parse; no SC2206 without
        //     shwordsplit/globsubst;
        //   `$hits[1]` / `$match[1]`: subscripts, no SC1087 or SC2128 without ksharrays;
        //     `match` is a special parameter, no SC2154;
        //   `*(.m+$days)` uses `days`, so no SC2034 on line 19;
        //   `(#i)`, `(#b)`, `<->`, `(a|b)` patterns parse; the case labels do not shadow
        //     each other (SC2221/2222) and nothing is "never matched" (SC2195).
        expect: &[(21, 2164)],
    },
    Script {
        name: "shwordsplit_legacy",
        file_name: "shwordsplit_legacy.zsh",
        text: include_str!("scripts/shwordsplit_legacy.zsh"),
        // Line 5 `setopt shwordsplit` turns splitting on for everything after it, so:
        // (12, SC2086) x2: `cp $f $backup_dir/` splits both words (b, shwordsplit).
        // (18, SC2068): `rm -- $@` re-splits the arguments (b).
        // (23, SC2048): `for a in $*` (b).
        // (30, SC2206): `parts=($1)` splits (b).
        // (40, SC2086): `ls $1` in `list_dir`, defined after `safe_copy` whose
        //     `setopt localoptions` did not leak (scope rule).
        // (43, SC2223): `: ${cache_dir:=$HOME/.cache}` (b).
        // Silent on purpose:
        //   line 36 `cp $1 $2` is inside `safe_copy`, which turns splitting off locally;
        //   line 11 `for f in $files_dir` and the quoted copies on line 14 and 24/31.
        expect: &[(12, 2086), (12, 2086), (18, 2068), (23, 2048), (30, 2206), (40, 2086), (43, 2223)],
    },
];

fn sorted(d: &[Diag]) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = d.iter().map(|x| (x.line, x.code)).collect();
    v.sort_unstable();
    v
}

fn expect_sorted(s: &Script) -> Vec<(u32, u32)> {
    let mut v = s.expect.to_vec();
    v.sort_unstable();
    v
}

case!(now, scripts_are_valid_zsh, {
    if !have_zsh() {
        return;
    }
    for s in SCRIPTS {
        assert!(zsh_n_ok(s.text), "zsh -n rejects scripts/{}", s.file_name);
    }
});

case!(now, scripts_are_plugin_sized, {
    for s in SCRIPTS {
        let n = s.text.lines().count();
        assert!((30..=80).contains(&n), "{} has {n} lines", s.file_name);
    }
});

macro_rules! plugin_case {
    ($mode:ident, $n:ident, $idx:expr) => {
        case!($mode, $n, {
            let s = &SCRIPTS[$idx];
            let got = zsh(s.text);
            assert_eq!(sorted(&got), expect_sorted(s), "(line, code) for scripts/{} under --shell=zsh", s.file_name);
        });
    };
}
plugin_case!(now, prompt_theme_reports_exactly_the_expected_findings, 0);
plugin_case!(now, completion_function_reports_exactly_the_expected_findings, 1);
plugin_case!(now, hook_plugin_reports_exactly_the_expected_findings, 2);
plugin_case!(now, emulate_library_reports_exactly_the_expected_findings, 3);
plugin_case!(now, shwordsplit_legacy_reports_exactly_the_expected_findings, 4);

case!(now, scripts_give_the_same_findings_when_zsh_is_inferred_from_the_file_name, {
    for s in SCRIPTS {
        let got = lint_named(s.file_name, &[], s.text);
        assert_eq!(sorted(&got), expect_sorted(s), "(line, code) for {} linted by file name only", s.file_name);
    }
});

case!(now, no_script_reports_a_parse_error_or_other_sc1xxx, {
    for s in SCRIPTS {
        assert!(parse_level(&zsh(s.text)).is_empty(), "{}: {:?}", s.name, parse_level(&zsh(s.text)));
    }
});

case!(now, scripts_never_report_class_d_codes, {
    const D: [u32; 13] = [2296, 2298, 2299, 2300, 2301, 2082, 2190, 2051, 2277, 2270, 2168, 2180, 2338];
    for s in SCRIPTS {
        let bad: Vec<u32> = codes(&zsh_with(&["--enable=all"], s.text)).into_iter().filter(|c| D.contains(c)).collect();
        assert!(bad.is_empty(), "{}: class (d) codes {bad:?}", s.name);
    }
});

case!(now, scripts_are_not_zsh_clean_for_other_shells_the_guard, {
    // The same text under bash must still fail to parse where it uses zsh syntax: the
    // zsh rules must not leak. (Only the scripts that use `{ cmd }` bodies or qualifiers.)
    for name in ["hook_plugin", "emulate_library"] {
        let s = SCRIPTS.iter().find(|s| s.name == name).unwrap();
        assert!(!parse_level(&shell("bash", s.text)).is_empty(), "{name} parses under bash");
    }
});
