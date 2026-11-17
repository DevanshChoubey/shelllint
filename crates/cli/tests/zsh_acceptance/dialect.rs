// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// Selecting zsh at every entry point, the one conformance record that touches zsh
// (`prop_readScript7`), and the guarantee that other shells are unaffected.
//
// Note on shebangs: while the temporary shebang shim is in place, a body that is zsh only
// because of its `#!/bin/zsh` line is consumed unanalysed. Tests use `--shell=zsh`, `-s zsh`,
// a `# shellcheck shell=zsh` directive or a `.zsh` file name, except the one ignored test
// (`shebang_alone_selects_zsh_and_lints_the_body`) that checks the shebang alone works.
use crate::support::*;

/// zsh-only syntax (C11): accepted by `zsh -n`, a parse error for every other shell.
const ZSH_ONLY: &str = "f() { echo \"$1\" }\nf a\n";

fn no_parse_errors(d: &[Diag], what: &str) {
    assert!(parse_level(d).is_empty(), "{what}: parse-level findings {:?}", parse_level(d));
}

case!(now, zsh_only_snippet_is_valid_zsh, {
    if have_zsh() {
        assert!(zsh_n_ok(ZSH_ONLY));
    }
});

case!(pend, entry_point_shell_flag, { no_parse_errors(&lint(&["--shell=zsh"], ZSH_ONLY), "--shell=zsh") });
case!(pend, entry_point_short_s_flag, { no_parse_errors(&lint(&["-s", "zsh"], ZSH_ONLY), "-s zsh") });
case!(pend, entry_point_directive, {
    no_parse_errors(&lint(&[], &format!("# shellcheck shell=zsh\n{ZSH_ONLY}")), "shell=zsh directive")
});
case!(pend, entry_point_directive_beats_a_bash_shebang, {
    no_parse_errors(&lint(&[], &format!("#!/bin/bash\n# shellcheck shell=zsh\n{ZSH_ONLY}")), "directive over shebang")
});
case!(pend, entry_point_fallback_shell, {
    no_parse_errors(&lint(&["--fallback-shell=zsh"], ZSH_ONLY), "--fallback-shell=zsh")
});
case!(pend, entry_point_file_names, {
    for name in ["plugin.zsh", "pure.zsh-theme", ".zshrc", ".zshenv", ".zprofile", ".zlogin", ".zlogout"] {
        no_parse_errors(&lint_named(name, &[], ZSH_ONLY), name);
    }
});
case!(now, entry_point_flag_beats_a_zsh_file_name_when_it_names_another_shell, {
    let got = lint_named("plugin.zsh", &["--shell=bash"], ZSH_ONLY);
    assert!(!parse_level(&got).is_empty(), "--shell=bash must win over the .zsh name: {:?}", pairs(&got));
});

// prop_readScript7 (the only conformance record that involves zsh):
//   #!/bin/zsh / # shellcheck disable=SC1071 / for f (a b); echo $f
// parses, and nothing is reported: in zsh `$f` is not split, so there is no SC2086.
const READ_SCRIPT7: &str = "#!/bin/zsh\n# shellcheck disable=SC1071\nfor f (a b); echo $f\n";

case!(pend, prop_read_script7_reports_nothing_under_shell_zsh, {
    let got = lint(&["--shell=zsh"], READ_SCRIPT7);
    assert!(got.is_empty(), "{:?}", pairs(&got));
});
case!(pend, prop_read_script7_reports_nothing_with_the_directive, {
    let got = lint(&[], "# shellcheck shell=zsh\nfor f (a b); echo $f\n");
    assert!(got.is_empty(), "{:?}", pairs(&got));
});
case!(pend, prop_read_script7_keeps_reporting_nothing_with_shwordsplit_off_spelled_out, {
    let got = lint(&["--shell=zsh"], "setopt noshwordsplit\nfor f (a b); echo $f\n");
    assert!(got.is_empty(), "{:?}", pairs(&got));
});

// The one test that uses a shebang alone (no flag, directive or file name). Ignored until
// the temporary shim that consumes shebang-only zsh bodies is removed at integration.
// Checks: the body is linted, zsh syntax parses, no SC1071, and `prop_readScript7` stays empty.
case!(pend, shebang_alone_selects_zsh_and_lints_the_body, {
    for shebang in ["#!/bin/zsh", "#!/usr/bin/zsh", "#!/usr/bin/env zsh"] {
        let got = lint(&[], &format!("{shebang}\necho \"$undefined_thing\"\n"));
        assert_eq!(lines(&got, 2154), [2], "{shebang}: {:?}", pairs(&got));
        assert!(!has(&got, 1071), "{shebang}: SC1071 means zsh is still unsupported");
        no_parse_errors(&lint(&[], &format!("{shebang}\n{ZSH_ONLY}")), shebang);
    }
    let got = lint(&[], READ_SCRIPT7);
    assert!(got.is_empty(), "prop_readScript7 under its bare shebang: {:?}", pairs(&got));
});

// ---- other shells are unaffected ---------------------------------------------------------

const OTHER_SHELLS: [&str; 5] = ["sh", "bash", "dash", "ksh", "busybox"];

case!(now, zsh_only_forms_are_still_parse_errors_everywhere_else, {
    for sh in OTHER_SHELLS {
        for s in [
            ZSH_ONLY,
            "sleep 1 &|\n",
            "diff =(echo a) =(echo b)\n",
            "{ :; } always { :; }\n",
            "for f (a b) echo $f\n",
            "() { echo hi; }\n",
            "echo a >>| f\n",
        ] {
            let got = shell(sh, s);
            assert!(!parse_level(&got).is_empty(), "{sh} must reject zsh syntax, got {:?} for:\n{s}", pairs(&got));
        }
    }
});

case!(now, option_commands_mean_nothing_to_other_shells_sc2086_still_fires, {
    for sh in OTHER_SHELLS {
        for pre in ["", "setopt noshwordsplit\n", "emulate -L zsh\n", "unsetopt shwordsplit\n"] {
            let got = shell(sh, &format!("{pre}echo $1\n"));
            let n = pre.lines().count() as u32 + 1;
            assert_eq!(lines(&got, 2086), [n], "{sh} with prefix {pre:?}: {:?}", pairs(&got));
        }
    }
});

case!(now, class_b_codes_keep_firing_for_the_other_shells, {
    let table: &[(u32, &str, &[&str])] = &[
        (2086, "echo $1\n", &OTHER_SHELLS),
        (2048, "for i in $*; do :; done\n", &OTHER_SHELLS),
        (2068, "foo $@\n", &OTHER_SHELLS),
        (2223, ": ${x:=$1}\n", &OTHER_SHELLS),
        (2206, "a=($1)\necho \"${a[@]}\"\n", &["bash"]),
        (2128, "a=(1 2)\necho \"$a\"\n", &["bash", "ksh"]),
        (2053, "[[ $1 == $2 ]]\n", &["bash", "ksh"]),
        (2254, "case $1 in $2) :;; esac\n", &OTHER_SHELLS),
        (2295, "x=\"${1#$2}\"\n", &["bash", "ksh"]),
        (2028, "echo \"a\\nb\"\n", &["sh", "bash", "ksh"]),
    ];
    for (code, s, shells) in table {
        for sh in *shells {
            let got = shell(sh, s);
            assert!(has(&got, *code), "SC{code} stopped firing under {sh} for:\n{s}{:?}", pairs(&got));
        }
    }
});

case!(now, class_d_codes_keep_firing_for_bash, {
    for (code, s) in [
        (2296, "echo \"${(L)1}\"\n"),
        (2298, "echo \"${${1}%y}\"\n"),
        (2299, "echo \"${${1%a}%b}\"\n"),
        (2300, "echo \"${$(date)#a}\"\n"),
        (2082, "echo \"${${1}}\"\n"),
        (2190, "typeset -A m\nm=(a 1 b 2)\n"),
        (2051, "for i in {1..$1}; do :; done\n"),
        (2277, "0=foo\n"),
        (2270, "1=foo\n"),
        (2168, "local x=1\n"),
        (2180, "m[1][2]=3\n"),
    ] {
        assert!(has(&shell("bash", s), code), "SC{code} stopped firing under bash for:\n{s}");
    }
});

case!(now, empty_and_comment_only_zsh_scripts_are_clean, {
    for s in ["", "\n", "# just a comment\n", "#!/bin/zsh\n"] {
        assert!(zsh(s).is_empty(), "{s:?}: {:?}", pairs(&zsh(s)));
    }
});

case!(now, zsh_does_not_crash_on_unfinished_input, {
    for s in ["f() {", "case $x in", "for x (", "[[ $a ==", "echo ${(", "=(", "() {", "always {", "echo $(( [##", "<-"]
    {
        let _ = zsh(s); // `lint` asserts exit status 0 or 1, so a crash or panic fails here
    }
});
