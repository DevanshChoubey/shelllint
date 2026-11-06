// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/Parser.hs (e11f964) lines 918-1107
// (readCondition, annotations, comments).

use shellcheck_parser::engine::{P, PR};
use shellcheck_parser::testing::{self, named_parser};

type Reader = fn(&mut P<'_>) -> PR<()>;

fn ok(r: Reader, s: &str) {
    let b = testing::is_ok(r, s);
    assert!(b, "expected clean parse of {s:?}");
}

fn warn(r: Reader, s: &str) {
    let b = testing::is_warning(r, s);
    assert!(b, "expected parse with warnings of {s:?}");
}

fn named(n: &str) -> Reader {
    named_parser(n).unwrap()
}

#[test]
fn read_condition() {
    let oks = [
        "[ \\( a = b \\) -a \\( c = d \\) ]",
        "[[ (a = b) || (c = d) ]]",
        "[[ $c = [[:alpha:].~-] ]]",
        "[[ $c =~ *foo* ]]",
        "[[ $c =~ f( ]] )* ]]",
        "[[ $c =~ a(b) ]]",
        "[[ $c =~ f( ($var ]]) )* ]]",
        "[[ $c =~ ^[yY]$ ]]",
        "[[ ${line} =~ ^[[:space:]]*# ]]",
        "[[ $l =~ ogg|flac ]]",
        "[ foo -a -f bar ]",
        "[[\na == b\n||\nc == d ]]",
        "[[\na == b  ||\nc == d ]]",
        "[[ a == b\n||\nc == d ]]",
        "[[ a == b ||\n c == d ]]",
        "[[ foo =~ ^fo{1,3}$ ]]",
        "[ foo '>' bar ]",
        "[ foo \">=\" bar ]",
        "[ foo \\< bar ]",
        "[[ ${file::1} = [-.\\|/\\\\] ]]",
        "[ ]",
        "[ '(' x \")\" ]",
        "[[ echo_rc -eq 0 ]]",
        "[[ $1 =~ ^(a\\ b)$ ]]",
        "[[ $1 =~ \\.a\\.(\\.b\\.)\\.c\\. ]]",
        "[[ -v arr[$var] ]]",
        "[[ lex.yy.c -ot program.l ]]",
        "[[ x = [\"$1\"] ]]",
        "[[ x = [*] ]]",
    ];
    for s in oks {
        ok(named("readCondition"), s);
    }
    warn(named("readCondition"), "[ a == b \n -o c == d ]");
}

#[test]
fn read_annotation() {
    let r = named("readAnnotation");
    for s in [
        "# shellcheck disable=1234,5678\n",
        "# shellcheck disable=SC1234 disable=SC5678\n",
        "# shellcheck disable=SC1234 source=/dev/null disable=SC5678\n",
        "# shellcheck disable=SC2002 # All cats are precious\n",
        "# shellcheck disable=SC1234 # shellcheck foo=bar\n",
        "# shellcheck disable=SC1000,SC2000-SC3000,SC1001\n",
        "# shellcheck disable=all\n",
        "# shellcheck source='foo bar' source-path=\"baz etc\"\n",
        "# shellcheck disable='SC1234,SC2345' enable=\"foo\" shell='bash'\n",
    ] {
        ok(r, s);
    }
    warn(r, "# shellcheck cats=dogs disable=SC1234\n");
    ok(named("(readAnnotationWithoutPrefix False)"), "external-sources='true'");
}

#[test]
fn read_any_comment() {
    ok(named("readAnyComment"), "# Comment");
}

#[test]
fn read_annotation_huge_code_does_not_overflow() {
    // The code saturates at i64::MAX; `from + 1` must not panic in debug builds.
    ok(named("readAnnotation"), "# shellcheck disable=SC99999999999999999999999\n");
    ok(named("readAnnotation"), "# shellcheck disable=99999999999999999999999-99999999999999999999999\n");
}
