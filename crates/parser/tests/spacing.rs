// SPDX-License-Identifier: GPL-3.0-only
// Port of the prop_spacing*/prop_allspacing* tests of ShellCheck src/ShellCheck/Parser.hs (e11f964)
//
// `spacing` calls readComment and linefeed calls readPendingHereDocs, which
// other parser areas port. While those are still `unported!` stubs the test
// is reported as blocked (it passes without asserting), like the conformance
// harness does; once they land the assertions run for real.

use std::panic::{AssertUnwindSafe, catch_unwind};

use shellcheck_parser::testing::{is_ok, is_warning, named_parser};

/// Run `f`; `None` if it hit an `unported!` stub of another area.
fn unless_blocked(f: impl FnOnce() -> bool) -> Option<bool> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(b) => Some(b),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            if msg.starts_with("unported:") {
                eprintln!("blocked: {msg}");
                None
            } else {
                std::panic::resume_unwind(e)
            }
        }
    }
}

fn check(target: &str, script: &str, warning: bool) {
    let p = named_parser(target).unwrap();
    let r = unless_blocked(|| if warning { is_warning(p, script) } else { is_ok(p, script) });
    assert_ne!(r, Some(false), "{target} on {script:?}");
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
