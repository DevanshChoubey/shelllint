// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1108-1611:
// the `prop_` tests of the words area (words, quotes, backticks, globs,
// extglobs, escapes, braces).
//
// Each test runs its target through `testing::named_parser`. A test whose
// call chain reaches another area's `unported!` stub is blocked and passes
// vacuously; reaching a stub of THIS area, or a wrong result, fails.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;

use shellcheck_parser::testing::{named_parser, parses_cleanly};

/// Haskell names of the readers this area ports.
const OWN: &[&str] = &[
    "readNormalWord",
    "readPatternWord",
    "readNormalishWord",
    "readIndexSpan",
    "checkPossibleTermination",
    "readNormalWordPart",
    "readSpacePart",
    "readDollarBracedWord",
    "readDollarBracedPart",
    "readDollarBracedLiteral",
    "readParamSubSpecialChar",
    "readProcSub",
    "readSingleQuoted",
    "readSingleQuotedLiteral",
    "readSingleQuotedPart",
    "readQuotedBackTicked",
    "readUnquotedBackTicked",
    "readBackTicked",
    "subParse",
    "inSeparateContext",
    "forgetOnFailure",
    "parseForgettingContext",
    "readDoubleQuoted",
    "suggestForgotClosingQuote",
    "doubleQuotedPart",
    "readDoubleLiteral",
    "readDoubleLiteralPart",
    "readNormalLiteral",
    "readGlob",
    "readNormalLiteralPart",
    "readNormalEscaped",
    "readExtglob",
    "readExtglobPart",
    "readSingleEscaped",
    "readDoubleEscaped",
    "readBraceEscaped",
    "readGenericLiteral",
    "readGenericLiteral1",
    "readGenericEscaped",
    "readBraced",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Expect {
    Ok,
    Warning,
    NotOk,
}

fn quiet_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| info.payload().downcast_ref::<&str>().copied())
                .unwrap_or("");
            if !msg.starts_with("unported: ") {
                default(info);
            }
        }));
    });
}

/// `isOk` / `isWarning` / `isNotOk` on a named reader.
fn check(target: &str, script: &str, want: Expect) {
    quiet_panics();
    let parser = named_parser(target).unwrap_or_else(|| panic!("no named parser {target}"));
    let r = catch_unwind(AssertUnwindSafe(|| parses_cleanly(parser, script)));
    match r {
        Ok(got) => {
            let got = match got {
                Some(true) => Expect::Ok,
                Some(false) => Expect::Warning,
                None => Expect::NotOk,
            };
            assert_eq!(got, want, "{target} {script:?}");
        }
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            let Some(name) = msg.strip_prefix("unported: ") else {
                std::panic::resume_unwind(e);
            };
            assert!(!OWN.contains(&name), "{target} {script:?} reached this area's stub {name}");
            eprintln!("blocked: {target} {script:?} reaches {name}");
        }
    }
}

macro_rules! props {
    ($($name:ident: $expect:ident $target:literal $script:literal;)*) => {
        $(
            #[test]
            #[allow(non_snake_case)]
            fn $name() {
                check($target, $script, Expect::$expect);
            }
        )*
    };
}

props! {
    prop_readNormalWord: Ok "readNormalWord" "'foo'\"bar\"{1..3}baz$(lol)";
    prop_readNormalWord2: Ok "readNormalWord" "foo**(foo)!!!(@@(bar))";
    prop_readNormalWord3: Ok "readNormalWord" "foo#";
    prop_readNormalWord4: Ok "readNormalWord" "$\"foo\"$'foo\nbar'";
    prop_readNormalWord5: Warning "readNormalWord" "${foo}}";
    prop_readNormalWord6: Ok "readNormalWord" "foo/{}";
    prop_readNormalWord7: Ok "readNormalWord" "foo\\\nbar";
    prop_readNormalWord8: Warning "readSubshell" "(foo\\ \nbar)";
    prop_readNormalWord9: Ok "readSubshell" "(foo\\ ;\nbar)";
    prop_readNormalWord10: Warning "readNormalWord" "\u{201C}hello\u{201D}";
    prop_readNormalWord11: Warning "readNormalWord" "\u{2018}hello\u{2019}";
    prop_readNormalWord12: Warning "readNormalWord" "hello\u{2018}";
    prop_readProcSub1: Ok "readProcSub" "<(echo test | wc -l)";
    prop_readProcSub2: Ok "readProcSub" "<(  if true; then true; fi )";
    prop_readProcSub3: Ok "readProcSub" "<( # nothing here \n)";
    prop_readSingleQuoted: Ok "readSingleQuoted" "'foo bar'";
    prop_readSingleQuoted2: Warning "readSingleQuoted" "'foo bar\\'";
    prop_readSingleQuoted4: Warning "readNormalWord" "'it's";
    prop_readSingleQuoted5: Warning "readSimpleCommand" "foo='bar\ncow 'arg";
    prop_readSingleQuoted6: Ok "readSimpleCommand" "foo='bar cow 'arg";
    prop_readSingleQuoted7: Ok "readSingleQuoted" "'foo\u{201C}bar'";
    prop_readSingleQuoted8: Warning "readSingleQuoted" "'foo\u{2018}bar'";
    prop_readBackTicked: Ok "(readBackTicked False)" "`ls *.mp3`";
    prop_readBackTicked2: Ok "(readBackTicked False)" "`grep \"\\\"\"`";
    prop_readBackTicked3: Warning "(readBackTicked False)" "\u{B4}grep \"\\\"\"\u{B4}";
    prop_readBackTicked4: Ok "readSimpleCommand" "`echo foo\necho bar`";
    prop_readBackTicked5: Ok "readSimpleCommand" "echo `foo`bar";
    prop_readBackTicked6: Warning "readSimpleCommand" "echo `foo\necho `bar";
    prop_readBackTicked7: Ok "readSimpleCommand" "`#inline comment`";
    prop_readBackTicked8: Ok "readSimpleCommand" "echo `#comment` \\\nbar baz";
    prop_readDoubleQuoted: Ok "readDoubleQuoted" "\"Hello $FOO\"";
    prop_readDoubleQuoted2: Ok "readDoubleQuoted" "\"$'\"";
    prop_readDoubleQuoted3: Ok "readDoubleQuoted" "\"\u{2018}hello\u{2019}\"";
    prop_readDoubleQuoted4: Warning "readSimpleCommand" "\"foo\nbar\"foo";
    prop_readDoubleQuoted5: Ok "readSimpleCommand" "lol \"foo\nbar\" etc";
    prop_readDoubleQuoted6: Ok "readSimpleCommand" "echo \"${ ls; }\"";
    prop_readDoubleQuoted7: Ok "readSimpleCommand" "echo \"${ ls;}bar\"";
    prop_readDoubleQuoted8: Warning "readDoubleQuoted" "\"\u{201C}hello\u{201D}\"";
    prop_readDoubleQuoted10: Ok "readDoubleQuoted" "\"foo\\\\n\"";
    prop_readGlob1: Ok "readGlob" "*";
    prop_readGlob2: Ok "readGlob" "[^0-9]";
    prop_readGlob3: Ok "readGlob" "[a[:alpha:]]";
    prop_readGlob4: Ok "readGlob" "[[:alnum:]]";
    prop_readGlob5: Ok "readGlob" "[^[:alpha:]1-9]";
    prop_readGlob6: Ok "readGlob" "[\\|]";
    prop_readGlob7: Ok "readGlob" "[^[]";
    prop_readGlob8: Ok "readGlob" "[*?]";
    prop_readGlob9: Ok "readGlob" "[!]^]";
    prop_readGlob10: Ok "readGlob" "[]]";
    prop_readExtglob1: Ok "readExtglob" "!(*.mp3)";
    prop_readExtglob2: Ok "readExtglob" "!(*.mp3|*.wmv)";
    prop_readExtglob4: Ok "readExtglob" "+(foo \\) bar)";
    prop_readExtglob5: Ok "readExtglob" "+(!(foo *(bar)))";
    prop_readExtglob6: Ok "readExtglob" "*(((||))|())";
    prop_readExtglob7: Ok "readExtglob" "*(<>)";
    prop_readExtglob8: Ok "readExtglob" "@(|*())";
    prop_readBraced: Ok "readBraced" "{1..4}";
    prop_readBraced2: Ok "readBraced" "{foo,bar,\"baz lol\"}";
    prop_readBraced3: Ok "readBraced" "{1,\\},2}";
    prop_readBraced4: Ok "readBraced" "{1,{2,3}}";
    prop_readBraced5: Ok "readBraced" "{JP{,E}G,jp{,e}g}";
    prop_readBraced6: Ok "readBraced" "{foo,bar,$((${var}))}";
    prop_readBraced7: NotOk "readBraced" "{}";
    prop_readBraced8: NotOk "readBraced" "{foo}";
}
