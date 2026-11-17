// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint extension. End-to-end smoke test of the zsh option
// tracker through the CLI: option commands of every shape must not crash the
// analysis. The state itself is tested in crates/analyzer/tests/zsh_options.rs.
#[path = "zsh_common.rs"]
mod zsh_common;

#[test]
fn option_commands_of_every_shape_analyse_without_crashing() {
    let scripts = [
        "setopt\nunsetopt\nset -o\nset +o\nemulate\n",
        "setopt $opts \"$x\" -o\nset -o $o -- $@\nemulate -L $shell -o $o\n",
        "emulate -L zsh -o extended_glob\nf() { setopt localoptions; emulate sh; }\nf\n",
        "options[extendedglob]=on\noptions[$k]=off\noptions[x y]=on\noptions=()\n",
        "f() { g; }\ng() { f; setopt nullglob; }\nf\n",
        "f() { f; }\nf\n",
        "( emulate -L zsh ) | { setopt kshglob; } &\n",
    ];
    for s in scripts {
        let _ = zsh_common::zsh_codes(s);
    }
}

#[test]
fn recursive_functions_that_leak_options_terminate() {
    let _ = zsh_common::zsh_codes("a() { b; setopt nullglob; }\nb() { a; setopt extendedglob; }\na\nb\n");
}
