// SPDX-License-Identifier: GPL-3.0-or-later
// Regression tests: the parser's `upper`/`lower`/`letter`/`alphaNum` follow
// GHC's general-category `Data.Char` predicates (Parser.hs e11f964), not
// Rust's derived Uppercase/Lowercase/Alphabetic properties. Expected output
// was taken from ShellCheck e11f964.

use shellcheck_core::interface::{ParseSpec, mocked_system_interface, new_parse_spec};
use shellcheck_parser::parse_script;

/// (line, column, code) of every parse comment, sorted.
fn parse_codes(script: &str) -> Vec<(i64, i64, i64)> {
    let sys = mocked_system_interface(vec![]);
    let spec = ParseSpec { filename: "-".into(), script: script.into(), ..new_parse_spec() };
    let mut out: Vec<_> = parse_script(&sys, &spec)
        .comments
        .iter()
        .map(|c| (c.start_pos.line, c.start_pos.column, c.comment.code))
        .collect();
    out.sort();
    out
}

#[test]
fn modifier_letter_is_not_a_variable_char() {
    // U+02B0 is Lm (Other_Lowercase): not `isLower`, so not a variable name.
    let got = parse_codes("#!/bin/bash\nfor ʰ in 1; do echo; done\n");
    assert!(got.contains(&(2, 1, 1073)) && got.contains(&(2, 5, 1072)), "{got:?}");
}

#[test]
fn titlecase_letter_is_a_variable_char() {
    // U+01C5 is Lt: `isUpper`, so `yǅ=2` is an assignment and `$ǅ` is not
    // a plain expansion start; U+00AA is Lo, so `xª=1` is not an assignment.
    let script = "#!/bin/bash\nxª=1\necho \"$xª\"\nyǅ=2\necho \"$yǅ\"\n";
    assert_eq!(parse_codes(script), vec![]);
    let sys = mocked_system_interface(vec![]);
    let spec = ParseSpec { filename: "-".into(), script: script.into(), ..new_parse_spec() };
    let pr = parse_script(&sys, &spec);
    let mut assigns = Vec::new();
    pr.ast.analyze(
        pr.root.unwrap(),
        &mut |id| {
            if let shellcheck_core::ast::Token::T_Assignment(_, name, ..) = pr.ast.get(id) {
                assigns.push(name.clone());
            }
        },
        &mut |_| {},
    );
    assert_eq!(assigns, vec!["yǅ".to_string()]);
}

#[test]
fn vowel_sign_is_not_a_letter_for_sc1011() {
    // U+0947 / U+093E are Mn/Mc: not `isAlpha`, so no SC1011.
    assert_eq!(parse_codes("#!/bin/bash\necho 'नमस्ते'world\n"), vec![]);
    let got = parse_codes("#!/bin/sh\necho 'abा'cd'\n");
    assert!(!got.iter().any(|c| c.2 == 1011), "{got:?}");
    // A letter before the quote still triggers it.
    let got = parse_codes("#!/bin/sh\necho 'abन'cd'\n");
    assert!(got.iter().any(|c| c.2 == 1011), "{got:?}");
}
