// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// Design section 5: under zsh only, codes whose text gives bash-only advice keep their
// meaning but get zsh-correct wording. SC2206 / SC2207 must advise zsh forms
// (`${(f)"$(cmd)"}`, `${(s: :)x}`, `read -rA`) and must not mention `mapfile` or
// `read -a`. Bash output must not change.
//
// zsh facts (zsh 5.9): `a=(${(f)"$(printf 'x\ny')"}); print $#a` prints 2;
// `x="a b"; a=(${(s: :)x}); print $#a` prints 2; `read -rA a <<<"a b"; print $#a` prints 2.
use crate::support::*;

fn zsh_forms(msg: &str) -> bool {
    ["${(f)", "(s:", "read -rA"].iter().any(|f| msg.contains(f))
}

fn bash_only_advice(msg: &str) -> Option<&'static str> {
    ["mapfile", "readarray", "read -a", "BASH_ARGV0"].into_iter().find(|w| msg.contains(w))
}

case!(pend, sc2207_zsh_advice_names_zsh_forms, {
    let got = zsh("a=($(date))\necho \"${a[@]}\"\n");
    let m = messages(&got, 2207);
    assert_eq!(m.len(), 1, "SC2207 once, got {:?}", pairs(&got));
    assert!(zsh_forms(&m[0]), "SC2207 text under zsh names no zsh form: {}", m[0]);
});
case!(pend, sc2207_zsh_advice_does_not_mention_mapfile_or_read_a, {
    let got = zsh("a=($(date))\necho \"${a[@]}\"\n");
    for m in messages(&got, 2207) {
        assert_eq!(bash_only_advice(&m), None, "bash-only advice in SC2207 under zsh: {m}");
    }
});
case!(pend, sc2206_zsh_advice_names_zsh_forms, {
    let got = zsh("setopt shwordsplit\na=($1)\necho \"${a[@]}\"\n");
    let m = messages(&got, 2206);
    assert_eq!(m.len(), 1, "SC2206 once, got {:?}", pairs(&got));
    assert!(zsh_forms(&m[0]), "SC2206 text under zsh names no zsh form: {}", m[0]);
});
case!(pend, sc2206_zsh_advice_does_not_mention_mapfile_or_read_a, {
    let got = zsh("setopt shwordsplit\na=($1)\necho \"${a[@]}\"\n");
    for m in messages(&got, 2206) {
        assert_eq!(bash_only_advice(&m), None, "bash-only advice in SC2206 under zsh: {m}");
    }
});
case!(pend, sc2206_on_explicit_equals_flag_uses_the_zsh_text_too, {
    let got = zsh("a=(${=1})\necho \"${a[@]}\"\n");
    let m = messages(&got, 2206);
    assert_eq!(m.len(), 1, "{:?}", pairs(&got));
    assert!(zsh_forms(&m[0]) && bash_only_advice(&m[0]).is_none(), "{}", m[0]);
});

const MAPFILE_2207: &str = "Prefer mapfile or read -a to split command output (or quote to avoid splitting).";
const MAPFILE_2206: &str = "Quote to prevent word splitting/globbing, or split robustly with mapfile or read -a.";
const KSH_2207: &str = "Prefer read -A or while read to split command output (or quote to avoid splitting).";
const KSH_2206: &str = "Quote to prevent word splitting/globbing, or split robustly with read -A or while read.";

// Exact texts recorded from the tree before zsh support: they must not change for the others.
case!(now, sc2207_and_sc2206_text_is_unchanged_for_bash_sh_dash_ksh, {
    for (sh, t2207, t2206) in [
        ("bash", MAPFILE_2207, MAPFILE_2206),
        ("sh", MAPFILE_2207, MAPFILE_2206),
        ("dash", MAPFILE_2207, MAPFILE_2206),
        ("ksh", KSH_2207, KSH_2206),
    ] {
        assert_eq!(messages(&shell(sh, "a=($(date))\n"), 2207), [t2207], "{sh} SC2207 text");
        assert_eq!(messages(&shell(sh, "a=($1)\n"), 2206), [t2206], "{sh} SC2206 text");
    }
});

case!(pend, sc2207_and_sc2206_zsh_text_differs_from_every_other_shell, {
    let z207 = messages(&zsh("a=($(date))\n"), 2207);
    let z206 = messages(&zsh("setopt shwordsplit\na=($1)\n"), 2206);
    assert_eq!(z207.len(), 1);
    assert_eq!(z206.len(), 1);
    for other in [MAPFILE_2207, KSH_2207] {
        assert_ne!(z207[0], other, "zsh SC2207 reuses another shell's text");
    }
    for other in [MAPFILE_2206, KSH_2206] {
        assert_ne!(z206[0], other, "zsh SC2206 reuses another shell's text");
    }
});

case!(pend, no_zsh_output_gives_bash_only_advice, {
    let scripts = [
        "a=($(date))\necho \"${a[@]}\"\n",
        "setopt shwordsplit\na=($1)\necho \"${a[@]}\"\n",
        "0=foo\n",
        "1=foo\n",
        "for i in {1..$1}; do echo \"$i\"; done\n",
        "typeset -A m\nm=(a 1 b 2)\n",
        "m[1][2]=3\n",
        "[[ $1 = $2 ]]\n",
        "echo hi |& cat\n",
        "x=$(cat file)\ny=($x)\n",
    ];
    for s in scripts {
        for d in zsh_with(&["--enable=all"], s) {
            assert_eq!(
                bash_only_advice(&d.message),
                None,
                "SC{} on line {} gives bash-only advice under zsh: {}",
                d.code,
                d.line,
                d.message
            );
        }
    }
});
