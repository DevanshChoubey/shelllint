// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/Parser.hs (e11f964) lines 918-1107
// (readCondition, annotations, comments).
//
// A test whose call chain reaches another area's `unported!` stub is skipped
// (the panic message starts with "unported"), matching the conformance
// harness's "blocked" classification; any other panic fails the test.

use std::panic::{AssertUnwindSafe, catch_unwind};

use shellcheck_parser::engine::{P, PR};
use shellcheck_parser::testing::{self, named_parser};

type Reader = fn(&mut P<'_>) -> PR<()>;

/// Some(result), or None when blocked on another area's stub.
fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            if msg.starts_with("unported") { None } else { std::panic::resume_unwind(e) }
        }
    }
}

fn ok(r: Reader, s: &str) {
    if let Some(b) = guarded(|| testing::is_ok(r, s)) {
        assert!(b, "expected clean parse of {s:?}");
    }
}

fn warn(r: Reader, s: &str) {
    if let Some(b) = guarded(|| testing::is_warning(r, s)) {
        assert!(b, "expected parse with warnings of {s:?}");
    }
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
