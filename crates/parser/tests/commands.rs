// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/Parser.hs (e11f964) lines 2054-2438
// and 2909-3127 (separators, simple commands, source, pipelines, and/or;
// assignments, arrays, let/eval/time suffixes).

use shellcheck_parser::engine::{P, PR};
use shellcheck_parser::testing::{self, named_parser};

type Reader = fn(&mut P<'_>) -> PR<()>;

fn ok(n: &str, s: &str) {
    let r: Reader = named_parser(n).unwrap();
    let b = testing::is_ok(r, s);
    assert!(b, "{n}: expected clean parse of {s:?}");
}

fn warn(n: &str, s: &str) {
    let r: Reader = named_parser(n).unwrap();
    let b = testing::is_warning(r, s);
    assert!(b, "{n}: expected parse with warnings of {s:?}");
}

#[test]
fn read_separator() {
    warn("readScript", "a &; b");
    ok("readScript", "a & b");
    warn("readScript", "a &amp; b");
    warn("readScript", "a &gt; file; b");
    warn("readScript", "curl https://example.com/?foo=moo&bar=cow");
}

#[test]
fn read_simple_command() {
    for s in [
        "echo test > file",
        "cmd &> file",
        "export foo=(bar baz)",
        "typeset -a foo=(lol)",
        "time if true; then echo foo; fi",
        "time -p ( ls -l; )",
        "\\ls",
        "\\:",
        "/\\* foo",
    ] {
        ok("readSimpleCommand", s);
    }
    for s in [
        "// Lol",
        "/* Lolbert */",
        "/**** Lolbert */",
        "elsif foo",
        "ElseIf foo",
        "elseif[$i==2]",
        "trap 'foo\"bar' INT",
    ] {
        warn("readSimpleCommand", s);
    }
}

#[test]
fn read_pipeline() {
    ok("readPipeline", "! cat /etc/issue | grep -i ubuntu");
    warn("readPipeline", "!cat /etc/issue | grep -i ubuntu");
    ok("readPipeline", "for f; do :; done|cat");
    ok("readPipeline", "! ! true");
    ok("readPipeline", "true | ! true");
}

#[test]
fn read_and_or() {
    ok("readAndOr", "grep -i lol foo || exit 1");
    ok("readAndOr", "# shellcheck disable=1\nfoo");
    ok("readAndOr", "# shellcheck disable=1\n# lol\n# shellcheck disable=3\nfoo");
}

#[test]
fn read_term() {
    ok("readTerm", "time ( foo; bar; )");
}

#[test]
fn read_assignment_word() {
    for s in [
        "a=42",
        "b=(1 2 3)",
        "b+=lol",
        "a[3$n'']=42",
        "a[4''$(cat foo)]=42",
        "IFS= ",
        "foo=",
        "foo=  ",
        "foo=  #bar",
        "foo=([a]=b [c] [d]= [e f )",
        "a[b <<= 3 + c]='thing'",
        "var=( (1 2) (3 4) )",
        "var=( 1 [2]=(3 4) )",
        "var=(1 [2]=(3 4))",
    ] {
        ok("readAssignmentWord", s);
    }
}
