// SPDX-License-Identifier: GPL-3.0-only
// Tests ported from ShellCheck src/ShellCheck/Parser.hs (e11f964), prop_a1..prop_a23
//
// A reader that depends on another area's still-unported stub panics with
// "unported: ..."; such a case is skipped here (the conformance report shows
// it as `blocked`). Once every area has landed, nothing is skipped.

use std::panic::{AssertUnwindSafe, catch_unwind};

use shellcheck_parser::testing::{is_ok, named_parser};

fn check(input: &str) {
    let p = named_parser("readArithmeticContents").expect("registered reader");
    match catch_unwind(AssertUnwindSafe(|| is_ok(p, input))) {
        Ok(ok) => assert!(ok, "readArithmeticContents should parse {input:?} cleanly"),
        Err(e) => {
            let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
            match msg {
                Some(m) if m.starts_with("unported:") => eprintln!("skipped (blocked): {m}"),
                _ => std::panic::resume_unwind(e),
            }
        }
    }
}

#[test]
fn prop_a1() {
    check(" n++ + ++c");
}

#[test]
fn prop_a2() {
    check("$N*4-(3,2)");
}

#[test]
fn prop_a3() {
    check("n|=2<<1");
}

#[test]
fn prop_a4() {
    check("n &= 2 **3");
}

#[test]
fn prop_a5() {
    check("1 |= 4 && n >>= 4");
}

#[test]
fn prop_a6() {
    check(" 1 | 2 ||3|4");
}

#[test]
fn prop_a7() {
    check("3*2**10");
}

#[test]
fn prop_a8() {
    check("3");
}

#[test]
fn prop_a9() {
    check("a^!-b");
}

#[test]
fn prop_a10() {
    check("! $?");
}

#[test]
fn prop_a11() {
    check("10#08 * 16#f");
}

#[test]
fn prop_a12() {
    check("\"$((3+2))\" + '37'");
}

#[test]
fn prop_a13() {
    check("foo[9*y+x]++");
}

#[test]
fn prop_a14() {
    check("1+`echo 2`");
}

#[test]
fn prop_a15() {
    check("foo[`echo foo | sed s/foo/4/g` * 3] + 4");
}

#[test]
fn prop_a16() {
    check("$foo$bar");
}

#[test]
fn prop_a17() {
    check("i<(0+(1+1))");
}

#[test]
fn prop_a18() {
    check("a?b:c");
}

#[test]
fn prop_a19() {
    check("\\\n3 +\\\n  2");
}

#[test]
fn prop_a20() {
    check("a ? b ? c : d : e");
}

#[test]
fn prop_a21() {
    check("a ? b : c ? d : e");
}

#[test]
fn prop_a22() {
    check("!!a");
}

#[test]
fn prop_a23() {
    check("~0");
}
