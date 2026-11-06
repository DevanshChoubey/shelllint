// SPDX-License-Identifier: GPL-3.0-only
// Tests ported from ShellCheck src/ShellCheck/Parser.hs (e11f964), prop_a1..prop_a23

use shellcheck_parser::testing::{is_ok, named_parser};

fn check(input: &str) {
    let p = named_parser("readArithmeticContents").expect("registered reader");
    assert!(is_ok(p, input), "readArithmeticContents should parse {input:?} cleanly");
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
