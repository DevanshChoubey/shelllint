// SPDX-License-Identifier: GPL-3.0-only
// Parser-backed tests of ShellCheck src/ShellCheck/AnalyzerLib.hs (e11f964)
// `getModifiedVariableCommand`; no Haskell counterpart (upstream tests it
// only through the checks).

use shellcheck_analyzer::analyzerlib::*;
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::only_literal_string;

/// `name:S` / `name:A` (DataString / DataArray) plus the source kind, and
/// the literal text of the token the variable is attributed to.
fn modified(s: &str) -> Vec<(String, String)> {
    let mut pr = p_script(s);
    let root = pr.root.expect("parses");
    pr.ast.compute_parents(root);
    let ast = &pr.ast;
    let mut cmd = None;
    ast.analyze(
        root,
        &mut |x| {
            if cmd.is_none() && matches!(ast.get(x), Token::T_SimpleCommand(..)) {
                cmd = Some(x)
            }
        },
        &mut |_| {},
    );
    let cmd: Id = cmd.unwrap();
    get_modified_variable_command(ast, cmd)
        .into_iter()
        .map(|(base, token, name, dt)| {
            assert_eq!(base, cmd);
            let (kind, src) = match &dt {
                DataType::DataString(s) => ("S", s),
                DataType::DataArray(s) => ("A", s),
            };
            let src = match src {
                DataSource::SourceFrom(_) => "from",
                DataSource::SourceExternal => "external",
                DataSource::SourceDeclaration => "declaration",
                DataSource::SourceInteger => "integer",
                DataSource::SourceChecked => "checked",
            };
            let tok = if token == cmd { "<cmd>".to_string() } else { only_literal_string(ast, token) };
            (format!("{name}:{kind}:{src}"), tok)
        })
        .collect()
}

fn names(s: &str) -> Vec<String> {
    modified(s).into_iter().map(|(n, _)| n).collect()
}

#[test]
fn declare_and_typeset() {
    assert_eq!(names("declare foo bar=1"), ["foo:S:declaration", "bar:S:from"]);
    assert_eq!(names("declare -a foo bar=(1)"), ["foo:A:declaration", "bar:A:from"]);
    assert_eq!(names("typeset -A m"), ["m:A:declaration"]);
    for s in ["declare -f foo", "declare -F foo", "declare -p foo"] {
        assert!(names(s).is_empty(), "{s}");
    }
    // Non-names are skipped, as are names starting with '-'.
    assert_eq!(names("declare -x 1x y"), ["y:S:declaration"]);
    assert_eq!(names("builtin declare z"), ["z:S:declaration"]);
}

#[test]
fn local_export_readonly() {
    assert_eq!(names("local a b=2"), ["a:S:declaration", "b:S:from"]);
    assert_eq!(names("export a=1"), ["a:S:from"]);
    assert!(names("export -f a").is_empty());
    assert!(names("readonly -p a").is_empty());
    assert_eq!(names("readonly a"), ["a:S:declaration"]);
}

#[test]
fn printf_dash_v() {
    assert_eq!(modified("printf -v foo '%s' x"), [("foo:S:from".to_string(), "foo".to_string())]);
    assert_eq!(names("printf -v 'arr[1]' x"), ["arr:A:from"]);
    assert_eq!(names("printf -v \"$name\" x"), ["!:S:from"]);
    assert!(names("printf '%s' x").is_empty());
}

#[test]
fn mapfile_and_readarray() {
    assert_eq!(modified("mapfile -t lines"), [("lines:A:external".to_string(), "lines".to_string())]);
    assert_eq!(modified("mapfile"), [("MAPFILE:A:external".to_string(), "<cmd>".to_string())]);
    assert_eq!(names("readarray -d '' -t arr < f"), ["arr:A:external"]);
    // Unknown flag: getGnuOpts fails, fall back to the last variable name.
    assert_eq!(names("mapfile -Q x y"), ["y:A:external"]);
}

#[test]
fn wait_dash_p() {
    assert_eq!(names("wait -n -p pid"), ["pid:S:integer"]);
    assert!(names("wait $pid").is_empty());
}

#[test]
fn set_positional_parameters() {
    assert_eq!(modified("set -- a b"), [("@:S:from".to_string(), "<cmd>".to_string())]);
    assert_eq!(names("set -e x"), ["@:S:from"]);
    assert!(names("set -o errexit").is_empty());
    assert!(names("set -e").is_empty());
}

#[test]
fn let_arguments() {
    // readLetSuffix parses arguments as arithmetic when it can; those are
    // TA_ tokens, which `oversimplify` drops (their assignments are found by
    // getModifiedVariables' TA_Assignment case instead).
    assert!(names("let a=1 b+=2").is_empty());
    // Arguments that are not arithmetic stay words: the name is the leading
    // variable characters after any +/-.
    assert_eq!(modified("let a=@b"), [("a:S:from".to_string(), "a=@b".to_string())]);
    assert_eq!(names("let ++c@"), ["c:S:from"]);
    assert!(names("let @1").is_empty());
    // `isVariableChar` admits digits, so this "name" is "1".
    assert_eq!(names("let 1@"), ["1:S:from"]);
}

#[test]
fn read_and_getopts() {
    assert_eq!(names("read -r a b"), ["a:S:external", "b:S:external"]);
    assert_eq!(names("read -ra arr"), ["arr:A:external"]);
    assert_eq!(names("getopts 'ab' opt"), ["opt:S:external"]);
}
