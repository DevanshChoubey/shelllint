// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the prop_spacing*/prop_allspacing* tests of ShellCheck src/ShellCheck/Parser.hs (e11f964)

use shellcheck_parser::testing::{is_ok, is_warning, named_parser};

fn check(target: &str, script: &str, warning: bool) {
    let p = named_parser(target).unwrap();
    let r = if warning { is_warning(p, script) } else { is_ok(p, script) };
    assert!(r, "{target} on {script:?}");
}

#[test]
fn prop_spacing1() {
    check("spacing", "  \\\n # Comment", false);
}

#[test]
fn prop_spacing2() {
    check("spacing", "# We can continue lines with \\", false);
}

#[test]
fn prop_spacing3() {
    check("spacing", "   \\\n #  --verbose=true \\", true);
}

#[test]
fn prop_allspacing() {
    check("allspacing", "#foo", false);
}

#[test]
fn prop_allspacing2() {
    check("allspacing", " #foo\n # bar\n#baz\n", false);
}

#[test]
fn prop_allspacing3() {
    check("allspacing", "#foo\n#bar\n#baz\n", false);
}
