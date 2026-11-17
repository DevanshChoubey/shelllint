// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint extension. End-to-end smoke test of the zsh option
// tracker through the CLI: option commands of every shape must not crash the
// analysis, and each one gives the codes recorded here (the same as before the
// zsh work, for the scripts that are not zsh-specific). The state itself is
// tested in crates/analyzer/tests/zsh_options.rs.
#[path = "zsh_common.rs"]
mod zsh_common;
use zsh_common::zsh_codes;

#[test]
fn option_commands_of_every_shape_analyse_without_crashing() {
    // (script, expected codes in report order)
    let cases: [(&str, &[u32]); 7] = [
        ("setopt\nunsetopt\nset -o\nset +o\nemulate\n", &[]),
        ("setopt $opts \"$x\" -o\nset -o $o -- $@\nemulate -L $shell -o $o\n", &[2154, 2154, 2154, 2154]),
        ("emulate -L zsh -o extended_glob\nf() { setopt localoptions; emulate sh; }\nf\n", &[]),
        ("options[extendedglob]=on\noptions[$k]=off\noptions[x y]=on\noptions=()\n", &[2154, 2004]),
        ("f() { g; }\ng() { f; setopt nullglob; }\nf\n", &[]),
        ("f() { f; }\nf\n", &[2264]),
        ("( emulate -L zsh ) | { setopt kshglob; } &\n", &[]),
    ];
    for (script, expected) in cases {
        assert_eq!(zsh_codes(script), expected, "codes for script:\n{script}");
    }
}

#[test]
fn recursive_functions_that_leak_options_terminate_with_no_findings() {
    let script = "a() { b; setopt nullglob; }\nb() { a; setopt extendedglob; }\na\nb\n";
    assert_eq!(zsh_codes(script), Vec::<u32>::new(), "codes for script:\n{script}");
}
