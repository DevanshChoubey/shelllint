// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only acceptance tests for zsh support (no ShellCheck counterpart).
//
// Test layer 5: independent acceptance tests for zsh support, written from the design
// (spec, plan, research) and from experiments with the real zsh 5.9, never from the
// implementation. Every test runs; a failure is a bug in the implementation unless
// zsh itself disagrees with the test.
//
#![allow(non_snake_case)] // test names keep zsh spellings such as `emulate_L_zsh`

// Modules (under tests/zsh_acceptance/):
//   support      run the binary, parse json1, drive real zsh when installed
//   constructs   C01-C29: parse, near-miss, other-shell guard
//   options      option spelling / scope / emulate, observed through SC2086 and friends
//   classes      spec section 4 classes (a) (b) (c) (d)
//   messages     zsh wording for SC2206 / SC2207
//   dialect      entry points, prop_readScript7, other shells unaffected
//   plugins      realistic plugin-shaped scripts with exact expected (code, line) lists

// `case!(now, name, { body })` is a plain test.
macro_rules! case {
    (now, $n:ident, $b:block) => {
        #[test]
        fn $n() $b
    };
}

macro_rules! code_case {
    (now, $n:ident, $c:expr, $s:expr, $e:expr) => {
        #[test]
        fn $n() {
            $crate::support::code_lint($c, $s, &$e)
        }
    };
}

mod support;

mod classes;
mod constructs;
mod dialect;
mod messages;
mod options;
mod plugins;
