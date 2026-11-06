// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the tests of ShellCheck src/ShellCheck/Checker.hs and
// src/ShellCheck/EditorConfig.hs (e11f964), plus pipeline tests checked
// against the oracle (koalaman/shellcheck v0.11.0-171-ge11f964).
//
// Tests whose outcome depends on a check that is not ported yet are
// `#[ignore = "needs <check>"]`; that includes `null` results that would
// pass vacuously without the check.

use shellcheck_checker::editorconfig::{
    EditorConfigFs, editor_config_directives, get_editor_config, invalid_directive_lines, invalid_root_lines,
    is_editor_config_rejection, is_editor_config_root, matches_glob, merge_configs, parse_editor_config,
    take_directory,
};
use shellcheck_checker::{check_script, check_script_codes, decode_string, sort_messages};
use shellcheck_core::interface::{
    CheckSpec, Code, Comment, ErrorMessage, MockedSystemInterface, Position, PositionedComment, Severity, Shell,
    SystemInterface, empty_check_spec, mock_rc_file, mocked_system_interface, new_positioned_comment,
};

// ---- Checker.hs test helpers ---------------------------------------------

type FindSource = fn(&str, Option<bool>, &[String], &str) -> String;

/// `mockedSystemInterface` with `siFindSource` / `siGetConfig` overridden.
struct TestSys {
    mock: MockedSystemInterface,
    find_source: Option<FindSource>,
    /// `Some(None)`: `siGetConfig` must not be called (`error "Fail"`).
    config: Option<Option<(String, String)>>,
}

impl TestSys {
    fn new(mock: MockedSystemInterface) -> Self {
        TestSys { mock, find_source: None, config: None }
    }
}

impl SystemInterface for TestSys {
    fn read_file(&self, external: Option<bool>, path: &str) -> Result<String, ErrorMessage> {
        self.mock.read_file(external, path)
    }
    fn find_source(&self, current: &str, external: Option<bool>, source_paths: &[String], name: &str) -> String {
        match self.find_source {
            Some(f) => f(current, external, source_paths, name),
            None => self.mock.find_source(current, external, source_paths, name),
        }
    }
    fn get_config(&self, file: &str) -> Option<(String, String)> {
        match &self.config {
            None => self.mock.get_config(file),
            Some(None) => panic!("Fail"),
            Some(Some(c)) => Some(c.clone()),
        }
    }
}

fn files(includes: &[(&str, &str)]) -> Vec<(String, String)> {
    includes.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn get_errors(sys: &dyn SystemInterface, spec: CheckSpec) -> Vec<Code> {
    let mut v: Vec<Code> = check_script(sys, &spec).comments.into_iter().map(|pc| pc.comment.code).collect();
    v.sort();
    v
}

fn check(src: &str) -> Vec<Code> {
    check_with_includes(&[], src)
}

fn check_with_spec(includes: &[(&str, &str)], spec: CheckSpec) -> Vec<Code> {
    get_errors(&mocked_system_interface(files(includes)), spec)
}

fn check_with_includes(includes: &[(&str, &str)], src: &str) -> Vec<Code> {
    check_with_spec(includes, CheckSpec { script: src.into(), excluded_warnings: vec![2148], ..empty_check_spec() })
}

fn check_recursive(includes: &[(&str, &str)], src: &str) -> Vec<Code> {
    check_with_spec(
        includes,
        CheckSpec { script: src.into(), excluded_warnings: vec![2148], check_sourced: true, ..empty_check_spec() },
    )
}

fn check_option_includes(includes: Option<Vec<Code>>, src: &str) -> Vec<Code> {
    check_with_spec(
        &[],
        CheckSpec { script: src.into(), included_warnings: includes, check_sourced: true, ..empty_check_spec() },
    )
}

fn check_with_rc(rc: &str, spec: CheckSpec) -> Vec<Code> {
    get_errors(&mock_rc_file(rc, mocked_system_interface(vec![])), spec)
}

fn check_with_includes_and_source_path(includes: &[(&str, &str)], mapper: FindSource, spec: CheckSpec) -> Vec<Code> {
    let sys = TestSys { find_source: Some(mapper), ..TestSys::new(mocked_system_interface(files(includes))) };
    get_errors(&sys, spec)
}

fn check_with_rc_includes_and_source_path(
    rc: &str,
    includes: &[(&str, &str)],
    mapper: FindSource,
    spec: CheckSpec,
) -> Vec<Code> {
    let sys = TestSys {
        find_source: Some(mapper),
        ..TestSys::new(mock_rc_file(rc, mocked_system_interface(files(includes))))
    };
    get_errors(&sys, spec)
}

/// shellcheck.* directives extracted from an EditorConfig file are merged
/// into the same "key=value" blob as .shellcheckrc. We simulate that here
/// by feeding editorConfigDirectives' output through siGetConfig.
fn check_with_editor_config(ec: &str, name: &str, src: &str) -> Vec<Code> {
    let blob = editor_config_directives(ec, name).unwrap_or_default();
    let sys = TestSys {
        config: Some(Some((".editorconfig".to_string(), blob))),
        ..TestSys::new(mocked_system_interface(files(&[("foo", src)])))
    };
    get_errors(&sys, CheckSpec { script: src.into(), excluded_warnings: vec![2148], ..empty_check_spec() })
}

fn spec(script: &str) -> CheckSpec {
    CheckSpec { script: script.into(), ..empty_check_spec() }
}

fn none() -> Vec<Code> {
    vec![]
}

// ---- Checker.hs props ----------------------------------------------------

#[test]
fn prop_editor_config_applies_known_shell() {
    assert_eq!(check_with_editor_config("[foo]\nshellcheck.shell=bash\n", "foo", "#!/bin/sh\necho \"hi\""), none());
}

#[test]
fn prop_editor_config_applies_disable() {
    assert_eq!(check_with_editor_config("[foo]\nshellcheck.disable=SC2086\n", "foo", "#!/bin/sh\necho $1"), none());
}

#[test]
fn prop_editor_config_unknown_shell_is_reported() {
    // An unknown shell can't be applied silently; it surfaces as a
    // config parse error (SC1134) rather than being dropped.
    assert_eq!(check_with_editor_config("[foo]\nshellcheck.shell=zsh\n", "foo", "#!/bin/sh\necho \"hi\""), vec![1134]);
}

#[test]
fn prop_editor_config_comment_value_is_reported() {
    assert_eq!(
        check_with_editor_config("[foo]\nshellcheck.disable=#abc\n", "foo", "#!/bin/sh\necho \"hi\""),
        vec![1134]
    );
}

#[test]
fn prop_editor_config_non_matching_section_ignored() {
    assert_eq!(check_with_editor_config("[bar]\nshellcheck.disable=SC2086\n", "foo", "#!/bin/sh\necho $1"), vec![2086]);
}

#[test]
fn prop_finds_parse_issue() {
    assert_eq!(check("echo \"$12\""), vec![1037]);
}

#[test]
fn prop_comment_disables_parse_issue1() {
    assert_eq!(check("#shellcheck disable=SC1037\necho \"$12\""), none());
}

#[test]
fn prop_comment_disables_parse_issue2() {
    assert_eq!(check("#shellcheck disable=SC1037\n#lol\necho \"$12\""), none());
}

#[test]
fn prop_finds_analysis_issue() {
    assert_eq!(check("echo $1"), vec![2086]);
}

#[test]
fn prop_comment_disables_analysis_issue1() {
    assert_eq!(check("#shellcheck disable=SC2086\necho $1"), none());
}

#[test]
fn prop_comment_disables_analysis_issue2() {
    assert_eq!(check("#shellcheck disable=SC2086\n#lol\necho $1"), none());
}

#[test]
fn prop_option_disables_issue1() {
    assert_eq!(
        get_errors(
            &mocked_system_interface(vec![]),
            CheckSpec { script: "echo $1".into(), excluded_warnings: vec![2148, 2086], ..empty_check_spec() }
        ),
        none()
    );
}

#[test]
fn prop_option_disables_issue2() {
    assert_eq!(
        get_errors(
            &mocked_system_interface(vec![]),
            CheckSpec { script: "echo \"$10\"".into(), excluded_warnings: vec![2148, 1037], ..empty_check_spec() }
        ),
        none()
    );
}

#[test]
fn prop_wont_parse_bad_shell() {
    assert_eq!(check("#!/usr/bin/python\ntrue $1\n"), vec![1071]);
}

#[test]
fn prop_option_disables_bad_shebang() {
    assert_eq!(
        get_errors(
            &mocked_system_interface(vec![]),
            CheckSpec {
                script: "#!/usr/bin/python\ntrue\n".into(),
                shell_type_override: Some(Shell::Sh),
                ..empty_check_spec()
            }
        ),
        none()
    );
}

#[test]
fn prop_annotation_disables_bad_shebang() {
    assert_eq!(check("#!/usr/bin/python\n# shellcheck shell=sh\ntrue\n"), none());
}

#[test]
fn prop_can_parse_dev_null() {
    assert_eq!(check("source /dev/null"), none());
}

#[test]
fn prop_fails_when_not_sourcing() {
    assert_eq!(check("source lol; echo \"$bar\""), vec![1091, 2154]);
}

#[test]
fn prop_fails_when_not_sourcing_parse_part() {
    // The parser half of prop_failsWhenNotSourcing.
    assert!(check("source lol; echo \"$bar\"").contains(&1091));
}

#[test]
fn prop_works_when_sourcing() {
    assert_eq!(check_with_includes(&[("lib", "bar=1")], "source lib; echo \"$bar\""), none());
}

#[test]
fn prop_works_when_sourcing_with_dash_dash() {
    assert_eq!(check_with_includes(&[("lib", "bar=1")], "source -- lib; echo \"$bar\""), none());
}

#[test]
fn prop_works_when_sourcing_with_dash_p() {
    assert_eq!(check_with_includes(&[("lib", "bar=1")], "source -p \"$MYPATH\" lib; echo \"$bar\""), none());
}

#[test]
fn prop_works_when_dotting() {
    assert_eq!(check_with_includes(&[("lib", "bar=1")], ". lib; echo \"$bar\""), none());
}

// FIXME: This should really be giving [1093], "recursively sourced"
#[test]
fn prop_no_infinite_sourcing() {
    assert_eq!(check_with_includes(&[("lib", "source lib")], "source lib"), none());
}

#[test]
fn prop_can_source_bad_syntax() {
    assert_eq!(check_with_includes(&[("lib", "for f; do")], "source lib; echo $1"), vec![1094, 2086]);
}

#[test]
fn prop_can_source_bad_syntax_parse_part() {
    // The parser half of prop_canSourceBadSyntax.
    assert!(check_with_includes(&[("lib", "for f; do")], "source lib; echo $1").contains(&1094));
}

#[test]
fn prop_cant_source_dynamic() {
    assert_eq!(check_with_includes(&[("lib", "")], ". \"$1\""), vec![1090]);
}

#[test]
fn prop_cant_source_dynamic2() {
    assert_eq!(check_with_includes(&[("lib", "")], "source ~/foo"), vec![1090]);
}

#[test]
fn prop_can_strip_prefix_and_source() {
    assert_eq!(check_with_includes(&[("./lib", "")], "source \"$MYDIR/lib\""), none());
}

#[test]
fn prop_can_strip_prefix_and_source2() {
    assert_eq!(
        check_with_includes(&[("./utils.sh", "")], "source \"$(dirname \"${BASH_SOURCE[0]}\")/utils.sh\""),
        none()
    );
}

#[test]
fn prop_can_source_dynamic_when_redirected() {
    assert_eq!(check_with_includes(&[("lib", "")], "#shellcheck source=lib\n. \"$1\""), none());
}

#[test]
fn prop_can_redirect_with_spaces() {
    assert_eq!(check_with_includes(&[("my file", "")], "#shellcheck source=\"my file\"\n. \"$1\""), none());
}

#[test]
fn prop_recursive_analysis() {
    assert_eq!(check_recursive(&[("lib", "echo $1")], "source lib"), vec![2086]);
}

#[test]
fn prop_recursive_parsing() {
    assert_eq!(check_recursive(&[("lib", "echo \"$10\"")], "source lib"), vec![1037]);
}

#[test]
fn prop_non_recursive_analysis() {
    assert_eq!(check_with_includes(&[("lib", "echo $1")], "source lib"), none());
}

#[test]
fn prop_non_recursive_parsing() {
    assert_eq!(check_with_includes(&[("lib", "echo \"$10\"")], "source lib"), none());
}

#[test]
fn prop_source_directive_doesnt_follow_file() {
    assert_eq!(
        check_with_includes(
            &[("foo", "source bar"), ("bar", "baz=3")],
            "#shellcheck source=foo\n. \"$1\"; echo \"$baz\""
        ),
        none()
    );
}

#[test]
fn prop_filewide_annotation_base() {
    assert_eq!(check("#!/bin/sh\necho $1"), vec![2086]);
}

#[test]
fn prop_filewide_annotation1() {
    assert_eq!(check("#!/bin/sh\n# shellcheck disable=2086\necho $1"), none());
}

#[test]
fn prop_filewide_annotation2() {
    assert_eq!(check("#!/bin/sh\n# shellcheck disable=2086\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation3() {
    assert_eq!(check("#!/bin/sh\n#unrelated\n# shellcheck disable=2086\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation4() {
    assert_eq!(check("#!/bin/sh\n# shellcheck disable=2086\n#unrelated\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation5() {
    assert_eq!(check("#!/bin/sh\n\n\n\n#shellcheck disable=2086\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation6() {
    assert_eq!(check("#shellcheck shell=sh\n#unrelated\n#shellcheck disable=2086\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation7() {
    assert_eq!(check("#!/bin/sh\n# shellcheck disable=2086\n#unrelated\ntrue\necho $1"), none());
}

#[test]
fn prop_filewide_annotation_base2() {
    assert_eq!(check("true\n[ $? == 0 ] && echo $1"), vec![2086, 2181]);
}

#[test]
fn prop_filewide_annotation8() {
    assert_eq!(
        check(
            "# Disable $? warning\n#shellcheck disable=SC2181\n# Disable quoting warning\n#shellcheck disable=2086\ntrue\n[ $? == 0 ] && echo $1"
        ),
        none()
    );
}

// #1181: -x disabled posix warning for 'source'
#[test]
fn prop_source_part_of_original_script() {
    assert!(check_with_includes(&[("./saywhat.sh", "echo foo")], "#!/bin/sh\nsource ./saywhat.sh").contains(&3046));
}

#[test]
fn prop_spin_bug1413() {
    assert_eq!(check("fun() {\n# shellcheck disable=SC2188\n> /dev/null\n}\n"), none());
}

#[test]
fn prop_deduces_type_from_extension() {
    assert_eq!(check_with_spec(&[], CheckSpec { filename: "file.ksh".into(), ..spec("(( 3.14 ))") }), none());
}

#[test]
fn prop_deduces_type_from_extension2() {
    assert_eq!(check_with_spec(&[], CheckSpec { filename: "file.bash".into(), ..spec("(( 3.14 ))") }), vec![2079]);
}

#[test]
fn prop_deduces_type_from_envrc_extension() {
    assert_eq!(check_with_spec(&[], CheckSpec { filename: ".envrc".into(), ..spec("(( 3.14 ))") }), vec![2079]);
}

#[test]
fn prop_can_disable_shebang_warning() {
    assert_eq!(
        check_with_spec(&[], CheckSpec { filename: "file.sh".into(), ..spec("#shellcheck disable=SC2148\nfoo") }),
        none()
    );
}

#[test]
fn prop_can_disable_all_warnings() {
    assert_eq!(
        check_with_spec(
            &[],
            CheckSpec {
                filename: "file.sh".into(),
                ..spec("#!/bin/sh\necho $1\n#shellcheck disable=all\necho `echo $1`")
            }
        ),
        vec![2086]
    );
}

#[test]
fn prop_can_disable_parse_errors() {
    assert_eq!(
        check_with_spec(
            &[],
            CheckSpec { filename: "file.sh".into(), ..spec("#shellcheck disable=SC1073,SC1072,SC2148\n()") }
        ),
        none()
    );
}

#[test]
fn prop_sh_extension_doesnt_matter() {
    assert_eq!(
        check_with_spec(&[], CheckSpec { filename: "file.sh".into(), ..spec("echo 'hello world'") }),
        vec![2148]
    );
}

#[test]
fn prop_sourced_file_uses_original_shell_extension() {
    assert_eq!(
        check_with_spec(
            &[("file.ksh", "(( 3.14 ))")],
            CheckSpec { filename: "file.bash".into(), check_sourced: true, ..spec("source file.ksh") }
        ),
        vec![2079]
    );
}

#[test]
fn prop_can_enable_optionals_with_spec() {
    assert_eq!(
        check_with_spec(
            &[],
            CheckSpec {
                filename: "file.sh".into(),
                optional_checks: vec!["avoid-nullary-conditions".into()],
                ..spec("#!/bin/sh\n[ \"$1\" ]")
            }
        ),
        vec![2244]
    );
}

#[test]
fn prop_option_includes1() {
    // expect 2086, but not included, so nothing reported
    assert_eq!(check_option_includes(Some(vec![2080]), "#!/bin/sh\n var='a b'\n echo $var"), none());
}

#[test]
fn prop_option_includes2() {
    // expect 2086, included, so it is reported
    assert_eq!(check_option_includes(Some(vec![2086]), "#!/bin/sh\n var='a b'\n echo $var"), vec![2086]);
}

#[test]
fn prop_option_includes3() {
    // expect 2086, no inclusions provided, so it is reported
    assert_eq!(check_option_includes(None, "#!/bin/sh\n var='a b'\n echo $var"), vec![2086]);
}

#[test]
fn prop_option_includes4() {
    // expect 2086 & 2154, only 2154 included, so only that's reported
    assert_eq!(check_option_includes(Some(vec![2154]), "#!/bin/sh\n var='a b'\n echo $var\n echo $bar"), vec![2154]);
}

#[test]
fn prop_reads_rc_file() {
    assert_eq!(check_with_rc("disable=2086", CheckSpec { ignore_rc: false, ..spec("#!/bin/sh\necho $1") }), none());
}

#[test]
fn prop_can_use_no_rc() {
    assert_eq!(check_with_rc("disable=2086", CheckSpec { ignore_rc: true, ..spec("#!/bin/sh\necho $1") }), vec![2086]);
}

#[test]
fn prop_no_rc_wont_look_at_file() {
    // The rc file is `error "Fail"`; with ignore_rc it is never read.
    // Haskell expects [2086]; without that check the result is empty, but
    // the point (no panic from reading the rc) is tested either way.
    let sys = TestSys { config: Some(None), ..TestSys::new(mocked_system_interface(vec![])) };
    let got = get_errors(&sys, CheckSpec { ignore_rc: true, ..spec("#!/bin/sh\necho $1") });
    assert!(got.iter().all(|&c| c == 2086), "{got:?}");
}

#[test]
fn prop_no_rc_wont_look_at_file_full() {
    let sys = TestSys { config: Some(None), ..TestSys::new(mocked_system_interface(vec![])) };
    assert_eq!(get_errors(&sys, CheckSpec { ignore_rc: true, ..spec("#!/bin/sh\necho $1") }), vec![2086]);
}

#[test]
fn prop_broken_rc_gets_warning() {
    assert_eq!(check_with_rc("rofl", CheckSpec { ignore_rc: false, ..spec("#!/bin/sh\necho $1") }), vec![1134, 2086]);
}

#[test]
fn prop_broken_rc_gets_warning_parse_part() {
    // The parser half of prop_brokenRcGetsWarning.
    assert!(check_with_rc("rofl", CheckSpec { ignore_rc: false, ..spec("#!/bin/sh\necho $1") }).contains(&1134));
}

#[test]
fn prop_can_enable_optionals_with_rc() {
    assert_eq!(check_with_rc("enable=avoid-nullary-conditions", spec("#!/bin/sh\n[ \"$1\" ]")), vec![2244]);
}

fn source_path_spec(script: &str) -> CheckSpec {
    CheckSpec { filename: "dir/myscript".into(), check_sourced: true, ..spec(script) }
}

fn unexpected(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
    panic!("{:?}", ("Unexpected", a, b, c, d))
}

#[test]
fn prop_source_path_redirects_name() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, d) {
            ("dir/myscript", "lib") => "foo/lib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_includes_and_source_path(&[("foo/lib", "echo $1")], f, source_path_spec("#!/bin/bash\nsource lib")),
        vec![2086]
    );
}

#[test]
fn prop_source_path_adds_annotation() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, c, d) {
            ("dir/myscript", [p], "lib") if p == "mypath" => "foo/lib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_includes_and_source_path(
            &[("foo/lib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\n# shellcheck source-path=mypath\nsource lib")
        ),
        vec![2086]
    );
}

#[test]
fn prop_source_path_works_with_spaces() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, c, d) {
            ("dir/myscript", [p], "lib") if p == "my path" => "foo/lib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_includes_and_source_path(
            &[("foo/lib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\n# shellcheck source-path='my path'\nsource lib")
        ),
        vec![2086]
    );
}

#[test]
fn prop_source_path_redirects_directive() {
    fn f(a: &str, _b: Option<bool>, _c: &[String], d: &str) -> String {
        match (a, d) {
            ("dir/myscript", "lib") => "foo/lib".into(),
            _ => "/dev/null".into(),
        }
    }
    assert_eq!(
        check_with_includes_and_source_path(
            &[("foo/lib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\n# shellcheck source=lib\nsource kittens")
        ),
        vec![2086]
    );
}

/// The resolver-argument halves of the source-path tests: the right
/// arguments reach `siFindSource` and the resolved file is sourced (its
/// parse error is reported with `-x`).
#[test]
fn prop_source_path_resolution_parse_part() {
    fn f1(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, d) {
            ("dir/myscript", "lib") => "foo/lib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    fn f2(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, c, d) {
            ("dir/myscript", [p], "lib") if p == "my path" => "foo/lib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    let inc = [("foo/lib", "echo \"$10\"")];
    assert_eq!(check_with_includes_and_source_path(&inc, f1, source_path_spec("#!/bin/bash\nsource lib")), vec![1037]);
    assert_eq!(
        check_with_includes_and_source_path(
            &inc,
            f2,
            source_path_spec("#!/bin/bash\n# shellcheck source-path='my path'\nsource lib")
        ),
        vec![1037]
    );
}

#[test]
fn prop_rc_can_allow_external_sources() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(true), "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "external-sources=true",
            &[("resolved/mylib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\nsource mylib")
        ),
        vec![2086]
    );
}

#[test]
fn prop_rc_can_deny_external_sources() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(false), "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "external-sources=false",
            &[("resolved/mylib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\nsource mylib")
        ),
        vec![2086]
    );
}

#[test]
fn prop_rc_can_leave_external_sources_unspecified() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", None, "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "",
            &[("resolved/mylib", "echo $1")],
            f,
            source_path_spec("#!/bin/bash\nsource mylib")
        ),
        vec![2086]
    );
}

/// The resolver-argument halves of the external-sources tests, using a
/// sourced parse error instead of 2086.
#[test]
fn prop_rc_external_sources_parse_part() {
    fn t(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(true), "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(false), "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    fn n(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", None, "mylib") => "resolved/mylib".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    let inc = [("resolved/mylib", "echo \"$10\"")];
    let s = || source_path_spec("#!/bin/bash\nsource mylib");
    assert_eq!(check_with_rc_includes_and_source_path("external-sources=true", &inc, t, s()), vec![1037]);
    assert_eq!(check_with_rc_includes_and_source_path("external-sources=false", &inc, f, s()), vec![1037]);
    assert_eq!(check_with_rc_includes_and_source_path("", &inc, n, s()), vec![1037]);
}

#[test]
fn prop_file_can_disable_external_sources() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(true), "withExternal") => "withExternal".into(),
            ("dir/myscript", Some(false), "withoutExternal") => "withoutExternal".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "external-sources=true",
            &[("withExternal", "echo $1"), ("withoutExternal", "_=`foo`")],
            f,
            source_path_spec(
                "#!/bin/bash\ntrue\nsource withExternal\n# shellcheck external-sources=false\nsource withoutExternal"
            )
        ),
        vec![2006, 2086]
    );
}

#[test]
fn prop_file_cannot_enable_external_sources() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", None, "foo") => "foo".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "",
            &[("foo", "true")],
            f,
            source_path_spec("#!/bin/bash\n# shellcheck external-sources=true\nsource foo")
        ),
        vec![1144]
    );
}

#[test]
fn prop_file_cannot_enable_external_sources2() {
    fn f(a: &str, b: Option<bool>, c: &[String], d: &str) -> String {
        match (a, b, d) {
            ("dir/myscript", Some(false), "foo") => "foo".into(),
            _ => unexpected(a, b, c, d),
        }
    }
    assert_eq!(
        check_with_rc_includes_and_source_path(
            "external-sources=false",
            &[("foo", "true")],
            f,
            source_path_spec("#!/bin/bash\n# shellcheck external-sources=true\nsource foo")
        ),
        vec![1144]
    );
}

#[test]
fn prop_rc_can_suppress_early_problems1() {
    assert_eq!(check_with_rc("disable=1071", spec("#!/bin/zsh\necho $1")), none());
}

#[test]
fn prop_rc_can_suppress_early_problems1_parse_part() {
    // Without the rc, the parser half reports 1071.
    assert_eq!(check_with_rc("", spec("#!/bin/zsh\necho $1")), vec![1071]);
    assert!(!check_with_rc("disable=1071", spec("#!/bin/zsh\necho $1")).contains(&1071));
}

#[test]
fn prop_rc_can_suppress_early_problems2() {
    assert_eq!(check_with_rc("disable=1104", spec("!/bin/bash\necho 'hello world'")), none());
}

#[test]
fn prop_source_with_here_doc_works() {
    assert_eq!(check_with_includes(&[("bar", "true\n")], "source bar << eof\nlol\neof"), none());
}

#[test]
fn prop_here_docs_are_parsed_without_trailing_linefeed() {
    assert!(check("cat << eof").contains(&1044));
}

#[test]
fn prop_here_docs_will_have_parsed_indices() {
    assert_eq!(check("#!/bin/bash\nmy_array=(a b)\ncat <<EOF >> ./test\n $(( 1 + my_array[1] ))\nEOF"), none());
}

#[test]
fn prop_rc_can_suppress_dfa() {
    assert_eq!(check_with_rc("extended-analysis=false", spec("#!/bin/sh\nexit; foo;")), none());
}

#[test]
fn prop_file_can_suppress_dfa() {
    assert_eq!(check_with_rc("", spec("#!/bin/sh\n# shellcheck extended-analysis=false\nexit; foo;")), none());
}

#[test]
fn prop_file_wins_when_suppressing_dfa1() {
    assert_eq!(
        check_with_rc("extended-analysis=true", spec("#!/bin/sh\n# shellcheck extended-analysis=false\nexit; foo;")),
        none()
    );
}

#[test]
fn prop_file_wins_when_suppressing_dfa2() {
    assert_eq!(
        check_with_rc("extended-analysis=false", spec("#!/bin/sh\n# shellcheck extended-analysis=true\nexit; foo;")),
        vec![2317]
    );
}

#[test]
fn prop_flag_wins_when_suppressing_dfa1() {
    assert_eq!(
        check_with_rc(
            "extended-analysis=false",
            CheckSpec {
                extended_analysis: Some(true),
                ..spec("#!/bin/sh\n# shellcheck extended-analysis=false\nexit; foo;")
            }
        ),
        vec![2317]
    );
}

#[test]
fn prop_flag_wins_when_suppressing_dfa2() {
    assert_eq!(
        check_with_rc(
            "extended-analysis=true",
            CheckSpec {
                extended_analysis: Some(false),
                ..spec("#!/bin/sh\n# shellcheck extended-analysis=true\nexit; foo;")
            }
        ),
        none()
    );
}

// ---- pipeline tests (expected values from the oracle) ---------------------

/// (code, line, column, end line, end column, file) of each comment, in order.
fn positioned(sys: &dyn SystemInterface, spec: CheckSpec) -> Vec<(Code, i64, i64, i64, i64, String)> {
    check_script(sys, &spec)
        .comments
        .into_iter()
        .map(|pc| {
            (
                pc.comment.code,
                pc.start_pos.line,
                pc.start_pos.column,
                pc.end_pos.line,
                pc.end_pos.column,
                pc.start_pos.file,
            )
        })
        .collect()
}

fn oracle_spec(file: &str, script: &str) -> CheckSpec {
    // shellcheck -f json1 --norc -o all <file>
    CheckSpec { filename: file.into(), ignore_rc: true, optional_checks: vec!["all".into()], ..spec(script) }
}

#[test]
fn crlf_reports_1017() {
    // oracle: crlf.sh 1:10-1:10 1017, 2:8-2:8 1017
    assert_eq!(
        positioned(&mocked_system_interface(vec![]), oracle_spec("crlf.sh", "#!/bin/sh\r\necho hi\r\n")),
        vec![(1017, 1, 10, 1, 10, "crlf.sh".into()), (1017, 2, 8, 2, 8, "crlf.sh".into())]
    );
}

#[test]
fn empty_file_no_comments() {
    // With 2148 excluded, as in every Checker.hs test.
    assert_eq!(check(""), none());
}

#[test]
fn empty_file_oracle_codes() {
    // oracle: empty.sh 1:1 2148
    assert_eq!(check_script_codes("", None), vec![2148]);
}

#[test]
fn nul_byte_matches_oracle() {
    // oracle: nul.sh 3:7-3:7 1037 (the NUL is an ordinary character)
    assert_eq!(
        positioned(&mocked_system_interface(vec![]), oracle_spec("nul.sh", "#!/bin/sh\necho a\0b\necho \"$12\"\n")),
        vec![(1037, 3, 7, 3, 7, "nul.sh".into())]
    );
}

#[test]
fn invalid_utf8_matches_oracle() {
    // oracle: inv.sh 2:9 1037 -- "\xc3\xa9" decodes to one char, the stray
    // "\xff" falls back to Latin-1 (one char).
    let script = decode_string(b"#!/bin/sh\necho \"\xc3\xa9\xff$12\"\n");
    assert_eq!(
        positioned(&mocked_system_interface(vec![]), oracle_spec("inv.sh", &script)),
        vec![(1037, 2, 9, 2, 9, "inv.sh".into())]
    );
    // oracle: sur.sh 2:10 1037 -- a surrogate (ED A0 80), an overlong NUL
    // (C0 80) and a truncated sequence (C3 then '$') are one char each.
    let script = decode_string(b"#!/bin/sh\necho \"\xed\xa0\x80\xc0\x80\xc3$12\"\n");
    assert_eq!(
        positioned(&mocked_system_interface(vec![]), oracle_spec("sur.sh", &script)),
        vec![(1037, 2, 10, 2, 10, "sur.sh".into())]
    );
}

#[test]
fn decode_string_cases() {
    assert_eq!(decode_string(b"abc"), "abc");
    assert_eq!(decode_string("é€😀".as_bytes()), "é€😀");
    assert_eq!(decode_string(b"\xff\xfe"), "\u{ff}\u{fe}");
    assert_eq!(decode_string(b"\xc3"), "\u{c3}");
    assert_eq!(decode_string(b"\xc3("), "\u{c3}(");
    assert_eq!(decode_string(b"\xc0\x80"), "\0");
    assert_eq!(decode_string(b"\xed\xa0\x80"), "\u{FFFD}");
    // F4 90 80 80 is above U+10FFFF: each byte falls back.
    assert_eq!(decode_string(b"\xf4\x90\x80\x80"), "\u{f4}\u{90}\u{80}\u{80}");
    assert_eq!(decode_string(b"\xf8\x80"), "\u{f8}\u{80}");
    assert_eq!(decode_string(b"\x80"), "\u{80}"); // stray continuation
}

#[test]
fn sourced_file_comments_use_its_file_name() {
    // A parse note and (once ported) analysis comments in a sourced file
    // carry the sourced file's name.
    let sys = mocked_system_interface(files(&[("lib", "echo \"$10\"")]));
    let got =
        positioned(&sys, CheckSpec { filename: "main".into(), check_sourced: true, ..spec("#!/bin/bash\nsource lib") });
    assert_eq!(got, vec![(1037, 1, 7, 1, 7, "lib".into())]);
}

fn pc(file: &str, line: i64, column: i64, severity: Severity, code: Code, message: &str) -> PositionedComment {
    let pos = Position { file: file.into(), line, column };
    PositionedComment {
        start_pos: pos.clone(),
        end_pos: pos,
        comment: Comment { severity, code, message: message.into() },
        ..new_positioned_comment()
    }
}

#[test]
fn sort_messages_order() {
    use Severity::*;
    let input = vec![
        pc("b", 1, 1, ErrorC, 1000, "x"),
        pc("a", 2, 10, StyleC, 2250, "m"),
        pc("a", 2, 10, InfoC, 2086, "m"),
        pc("a", 2, 10, WarningC, 2154, "m"),
        pc("a", 2, 4, InfoC, 2021, "m"),
        pc("a", 2, 4, InfoC, 2020, "z"),
        pc("a", 2, 4, InfoC, 2020, "a"),
        pc("a", 1, 9, ErrorC, 1, "m"),
    ];
    let got: Vec<(String, i64, i64, Code, String)> = sort_messages(input)
        .into_iter()
        .map(|p| (p.start_pos.file, p.start_pos.line, p.start_pos.column, p.comment.code, p.comment.message))
        .collect();
    let want: Vec<(String, i64, i64, Code, String)> = [
        ("a", 1, 9, 1, "m"),
        ("a", 2, 4, 2020, "a"),
        ("a", 2, 4, 2020, "z"),
        ("a", 2, 4, 2021, "m"),
        // oracle order at one position (wc.sh 2:10): warning, info, style
        ("a", 2, 10, 2154, "m"),
        ("a", 2, 10, 2086, "m"),
        ("a", 2, 10, 2250, "m"),
        ("b", 1, 1, 1000, "x"),
    ]
    .into_iter()
    .map(|(f, l, c, k, m)| (f.to_string(), l, c, k, m.to_string()))
    .collect();
    assert_eq!(got, want);
}

#[test]
fn min_severity_filters() {
    // oracle (-s bash): 2:3 1009 (info), 2:12 1073 (error), 3:1 1072 (error)
    let script = "for f in a; do\n  if true; then\n";
    let all = check_script(
        &mocked_system_interface(vec![]),
        &CheckSpec { shell_type_override: Some(Shell::Bash), ..spec(script) },
    );
    let got: Vec<(Code, i64, i64)> =
        all.comments.iter().map(|p| (p.comment.code, p.start_pos.line, p.start_pos.column)).collect();
    assert_eq!(got, vec![(1009, 2, 3), (1073, 2, 12), (1072, 3, 1)]);
    let sev: Vec<Severity> = all.comments.iter().map(|p| p.comment.severity).collect();
    assert!(sev.contains(&Severity::InfoC) && sev.contains(&Severity::ErrorC), "{:?}", all.comments);
    let errors =
        check_script(&mocked_system_interface(vec![]), &CheckSpec { min_severity: Severity::ErrorC, ..spec(script) });
    assert!(!errors.comments.is_empty());
    assert!(errors.comments.iter().all(|p| p.comment.severity == Severity::ErrorC));
}

#[test]
fn same_position_order() {
    // oracle: tr.sh 2:4-2:10 2020 (info), then 2:4-2:10 2021 (info)
    assert_eq!(
        positioned(&mocked_system_interface(vec![]), oracle_spec("tr.sh", "#!/bin/bash\ntr '[aa]' x\n")),
        vec![(2020, 2, 4, 2, 10, "tr.sh".into()), (2021, 2, 4, 2, 10, "tr.sh".into())]
    );
}

#[test]
fn checks_crate_is_linked() {
    // checkEchoWc is in the registry and runs through the pipeline
    // (oracle: wc.sh 2:5-2:22 2000).
    let entries = shellcheck_analyzer::api::registered(shellcheck_checks::CHECKS, "node");
    assert!(entries.iter().any(|e| e.module == "Analytics.hs" && e.name == "checkEchoWc"));
    assert!(check_script_codes("#!/bin/bash\nn=$(echo $foo | wc -c)\n", None).contains(&2000));
}

#[test]
fn check_script_codes_matches_oracle_flags() {
    // -s bash suppresses the shebang/shell problems; parse notes pass through.
    assert_eq!(check_script_codes("echo \"$12\"", Some(Shell::Bash)), vec![1037]);
    assert_eq!(check_script_codes("#!/usr/bin/python\ntrue\n", None), vec![1071]);
}

#[test]
fn deeply_nested_input_does_not_overflow() {
    let script = "$(".repeat(5000) + &")".repeat(5000);
    let _ = check_script_codes(&script, None);
}

// ---- EditorConfig.hs props -------------------------------------------------

#[test]
fn prop_glob_star() {
    assert!(matches_glob("*.ebuild", "foo.ebuild"));
}
#[test]
fn prop_glob_brace_ext() {
    assert!(matches_glob("*.{ebuild,eclass}", "foo.eclass"));
}
#[test]
fn prop_glob_brace_ext2() {
    assert!(matches_glob("*.{ebuild,eclass}", "foo.ebuild"));
}
#[test]
fn prop_glob_brace_name() {
    assert!(matches_glob("{PKGBUILD,APKBUILD}", "PKGBUILD"));
}
#[test]
fn prop_glob_brace_name2() {
    assert!(matches_glob("{PKGBUILD,APKBUILD}", "APKBUILD"));
}
#[test]
fn prop_glob_no_match() {
    assert!(!matches_glob("*.ebuild", "foo.txt"));
}
#[test]
fn prop_glob_question() {
    assert!(matches_glob("foo?.sh", "food.sh"));
}
#[test]
fn prop_glob_class() {
    assert!(matches_glob("foo[0-9].sh", "foo1.sh"));
}
#[test]
fn prop_glob_class_neg() {
    assert!(!matches_glob("foo[!0-9].sh", "foo1.sh"));
}
// Patterns without a path separator should match at any depth.
#[test]
fn prop_glob_any_depth() {
    assert!(matches_glob("*.sh", "sub/dir/foo.sh"));
}
#[test]
fn prop_glob_any_depth_plain() {
    assert!(matches_glob("foo", "sub/foo"));
}
// Patterns with a path separator are only matched against the full
// relative path.
#[test]
fn prop_glob_with_slash_no_match() {
    assert!(!matches_glob("sub/*.sh", "other/foo.sh"));
}
#[test]
fn prop_glob_with_slash_match() {
    assert!(matches_glob("sub/*.sh", "sub/foo.sh"));
}
// Numeric range expansion
#[test]
fn prop_glob_range() {
    assert!(matches_glob("file{1..3}.sh", "file2.sh"));
}
#[test]
fn prop_glob_range_start() {
    assert!(matches_glob("file{1..3}.sh", "file1.sh"));
}
#[test]
fn prop_glob_range_end() {
    assert!(matches_glob("file{1..3}.sh", "file3.sh"));
}
#[test]
fn prop_glob_range_no_match() {
    assert!(!matches_glob("file{1..3}.sh", "file4.sh"));
}
#[test]
fn prop_glob_range_negative() {
    assert!(matches_glob("file{-2..0}.sh", "file-1.sh"));
}
#[test]
fn prop_glob_range_descending() {
    assert!(!matches_glob("file{3..1}.sh", "file2.sh"));
}
#[test]
fn prop_glob_literal_dots() {
    assert!(!matches_glob("file{1..3}.sh", "file1..3.sh"));
}
// Empty brace alternatives (e.g. 'foo{,bar}') make the group optional.
#[test]
fn prop_glob_brace_empty_alt() {
    assert!(matches_glob("foo{,bar}", "foo"));
}
#[test]
fn prop_glob_brace_empty_alt2() {
    assert!(matches_glob("foo{,bar}", "foobar"));
}
#[test]
fn prop_glob_brace_empty_alt_no_match() {
    assert!(!matches_glob("foo{,bar}", "foobaz"));
}
// Nested braces: '{foo,ba{r,z}}' matches foo, bar and baz.
#[test]
fn prop_glob_brace_nested1() {
    assert!(matches_glob("{foo,ba{r,z}}", "foo"));
}
#[test]
fn prop_glob_brace_nested2() {
    assert!(matches_glob("{foo,ba{r,z}}", "bar"));
}
#[test]
fn prop_glob_brace_nested3() {
    assert!(matches_glob("{foo,ba{r,z}}", "baz"));
}
#[test]
fn prop_glob_brace_nested_no_match() {
    assert!(!matches_glob("{foo,ba{r,z}}", "baq"));
}

#[test]
fn prop_parse_editor_config1() {
    assert_eq!(
        parse_editor_config("[*.{ebuild,eclass}]\nshellcheck.shell=bash\nshellcheck.disable=SC2034\n", "foo.ebuild"),
        "\nshell=bash\ndisable=SC2034\n"
    );
}
#[test]
fn prop_parse_editor_config2() {
    assert_eq!(parse_editor_config("[*.{ebuild,eclass}]\nshellcheck.shell=bash\n", "foo.txt"), "");
}
#[test]
fn prop_parse_editor_config3() {
    assert_eq!(
        parse_editor_config("[{PKGBUILD,APKBUILD}]\nshellcheck.disable=SC2034\n", "PKGBUILD"),
        "\ndisable=SC2034\n"
    );
}
#[test]
fn prop_parse_editor_config4() {
    assert_eq!(
        parse_editor_config("root = true\n[*.sh]\nindent_style = space\nshellcheck.shell=bash\n", "foo.sh"),
        "\n\n\nshell=bash\n"
    );
}
// A later, more specific section overrides an earlier, more general
// one for the same key.
#[test]
fn prop_parse_editor_config5() {
    assert_eq!(
        parse_editor_config("[*]\nshellcheck.shell=sh\n\n[foo]\nshellcheck.shell=bash\n", "foo"),
        "\n\n\n\nshell=bash\n"
    );
}
// Non-conflicting keys from earlier and later sections are all kept.
#[test]
fn prop_parse_editor_config6() {
    assert_eq!(
        parse_editor_config("[*]\nshellcheck.shell=sh\n\n[foo]\nshellcheck.disable=SC2034\n", "foo"),
        "\nshell=sh\n\n\ndisable=SC2034\n"
    );
}
#[test]
fn prop_parse_editor_config_unknown_shell() {
    assert_eq!(parse_editor_config("[*]\nshellcheck.shell=zsh\n", "foo"), "");
}
#[test]
fn prop_parse_editor_config_empty_shell() {
    assert_eq!(parse_editor_config("[*]\nshellcheck.shell=\n", "foo"), "\nshell=\n");
}
#[test]
fn prop_parse_editor_config_empty_disable() {
    assert_eq!(parse_editor_config("[*]\nshellcheck.disable=\n", "foo"), "");
}
#[test]
fn prop_parse_editor_config_embedded_comment() {
    assert_eq!(parse_editor_config("[*]\nshellcheck.disable=SC2148 #abc\n", "foo"), "");
}
#[test]
fn prop_parse_editor_config_inline_comment() {
    assert_eq!(parse_editor_config("[*]\nshellcheck.shell=bash # inline\n", "foo"), "");
}
// Full-line comments starting on first non-ws char are stripped.
#[test]
fn prop_parse_editor_config_line_comment() {
    assert_eq!(parse_editor_config("[*]\n# shellcheck.shell=bash\n", "foo"), "");
}
#[test]
fn prop_parse_editor_config_semicolon_comment() {
    assert_eq!(parse_editor_config("[*]\n; shellcheck.shell=bash\n", "foo"), "");
}
#[test]
fn prop_parse_editor_config_brace_empty() {
    assert_eq!(parse_editor_config("[foo{,bar}]\nshellcheck.shell=sh\n", "foo"), "\nshell=sh\n");
}
#[test]
fn prop_parse_editor_config_brace_nested() {
    assert_eq!(parse_editor_config("[{foo,ba{r,z}}]\nshellcheck.shell=sh\n", "baz"), "\nshell=sh\n");
}
#[test]
fn prop_is_editor_config_root_empty() {
    assert!(!is_editor_config_root("root =\n"));
}
#[test]
fn prop_is_editor_config_root_false() {
    assert!(!is_editor_config_root("root = false\n"));
}
#[test]
fn prop_is_editor_config_root_true() {
    assert!(is_editor_config_root("root = TRUE\n"));
}
#[test]
fn prop_invalid_root_lines_empty() {
    assert_eq!(invalid_root_lines("root =\n"), vec![1]);
}
#[test]
fn prop_invalid_root_lines_true() {
    assert_eq!(invalid_root_lines("root = true\n"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_root_lines_false() {
    assert_eq!(invalid_root_lines("root = false\n"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_root_lines_in_section() {
    assert_eq!(invalid_root_lines("[*]\nroot = true\n"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_unknown_shell() {
    assert_eq!(invalid_directive_lines("[*]\nshellcheck.shell=zsh\n", "foo"), vec![2]);
}
#[test]
fn prop_invalid_directive_lines_empty_shell() {
    assert_eq!(invalid_directive_lines("[*]\nshellcheck.shell=\n", "foo"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_known_shell() {
    assert_eq!(invalid_directive_lines("[*]\nshellcheck.shell=bash\n", "foo"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_hash_value() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable = #abc\n", "foo"), vec![2]);
}
#[test]
fn prop_invalid_directive_lines_semicolon_value() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable = ;abc\n", "foo"), vec![2]);
}
#[test]
fn prop_invalid_directive_lines_embedded_hash_value() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable = SC2148 #abc\n", "foo"), vec![2]);
}
#[test]
fn prop_invalid_directive_lines_embedded_semicolon_value() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable = SC2148 ;abc\n", "foo"), vec![2]);
}
#[test]
fn prop_invalid_directive_lines_empty_value_not_invalid() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable =\n", "foo"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_plain_value() {
    assert_eq!(invalid_directive_lines("[foo]\nshellcheck.disable = abc\n", "foo"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_no_match() {
    assert_eq!(invalid_directive_lines("[*.txt]\nshellcheck.shell=zsh\n", "foo"), Vec::<usize>::new());
}
#[test]
fn prop_invalid_directive_lines_matching_section() {
    assert_eq!(
        invalid_directive_lines("[*.txt]\nshellcheck.shell=zsh\n[foo]\nshellcheck.shell=bash\n", "foo"),
        Vec::<usize>::new()
    );
}

#[test]
fn editor_config_directives_cases() {
    assert_eq!(editor_config_directives("[foo]\nshellcheck.shell=bash\n", "foo"), Some("\nshell=bash\n".into()));
    assert_eq!(editor_config_directives("[bar]\nshellcheck.shell=bash\n", "foo"), None);
    assert_eq!(
        editor_config_directives("[foo]\nshellcheck.shell=zsh\nshellcheck.disable=#x\n", "foo"),
        Some("\ninvalid editorconfig value\n\n\ninvalid editorconfig value\n".into())
    );
}

// ---- shellcheck.hs EditorConfig lookup ---------------------------------

struct MockFs(Vec<(&'static str, &'static str)>, Option<&'static str>);

impl EditorConfigFs for MockFs {
    fn normalize(&self, path: &str) -> String {
        if path == "." { "/home/u/proj".into() } else { path.into() }
    }
    fn read_config(&self, file: &str) -> Option<(String, String)> {
        self.0.iter().find(|(n, _)| *n == file).map(|(n, c)| (n.to_string(), c.to_string()))
    }
    fn global_editor_config_path(&self) -> Option<String> {
        self.1.map(String::from)
    }
}

#[test]
fn take_directory_cases() {
    assert_eq!(take_directory("foo/bar"), "foo");
    assert_eq!(take_directory("/foo"), "/");
    assert_eq!(take_directory("/"), "/");
    assert_eq!(take_directory("foo"), ".");
    assert_eq!(take_directory("foo//bar"), "foo");
}

#[test]
fn get_editor_config_walks_parents() {
    let fs = MockFs(
        vec![
            ("/home/u/proj/sub/.editorconfig", "[*.sh]\nshellcheck.disable=SC2034\n"),
            ("/home/u/proj/.editorconfig", "root = true\n[sub/*.sh]\nshellcheck.shell=bash\n"),
            ("/home/u/.editorconfig", "[*]\nshellcheck.shell=sh\n"),
        ],
        None,
    );
    // The root file stops the search; both contribute; first file names it.
    assert_eq!(
        get_editor_config(&fs, "/home/u/proj/sub/a.sh"),
        Some(("/home/u/proj/sub/.editorconfig".into(), "\ndisable=SC2034\n\n\nshell=bash\n".into()))
    );
    // A relative name is resolved through normalize(takeDirectory).
    let fs = MockFs(vec![("/home/u/proj/.editorconfig", "root=true\n[a.sh]\nshellcheck.shell=bash\n")], None);
    assert_eq!(get_editor_config(&fs, "a.sh"), Some(("/home/u/proj/.editorconfig".into(), "\n\nshell=bash\n".into())));
    assert_eq!(get_editor_config(&fs, "b.sh"), None);
}

#[test]
fn get_editor_config_global_and_bad_root() {
    let fs = MockFs(
        vec![("/p/.editorconfig", "root =\n"), ("/xdg/editorconfig.ini", "[*]\nshellcheck.shell=sh\n")],
        Some("/xdg/editorconfig.ini"),
    );
    assert_eq!(
        get_editor_config(&fs, "/p/a.sh"),
        Some(("/p/.editorconfig".into(), "invalid editorconfig value\n\nshell=sh\n".into()))
    );
}

#[test]
fn merge_configs_cases() {
    let rc = Some((".shellcheckrc".to_string(), "disable=SC1000".to_string()));
    let ec = Some((".editorconfig".to_string(), "\nshell=bash\n".to_string()));
    let bad = Some((".editorconfig".to_string(), "\ninvalid editorconfig value\n".to_string()));
    assert_eq!(merge_configs(None, None), None);
    assert_eq!(merge_configs(rc.clone(), None), rc);
    assert_eq!(merge_configs(None, ec.clone()), ec);
    assert_eq!(
        merge_configs(rc.clone(), ec.clone()),
        Some((".shellcheckrc".into(), "disable=SC1000\n\nshell=bash\n".into()))
    );
    assert_eq!(merge_configs(rc, bad.clone()), bad);
    assert!(is_editor_config_rejection("\n  \ninvalid editorconfig value\n"));
    assert!(!is_editor_config_rejection("shell=sh\n"));
}

// ---- fix round 1 -------------------------------------------------------------

/// Parse `main` (`source lib` with `-x`) and return the result.
fn parse_with_lib(main: &str, lib: &str) -> shellcheck_parser::ParseResult {
    let sys = mocked_system_interface(files(&[("lib", lib)]));
    let spec = shellcheck_core::interface::ParseSpec {
        filename: "main".into(),
        script: main.into(),
        check_sourced: true,
        ..shellcheck_core::interface::new_parse_spec()
    };
    shellcheck_parser::parse_script(&sys, &spec)
}

fn find_literal(pr: &shellcheck_parser::ParseResult, s: &str) -> shellcheck_core::ast::Id {
    let mut found = None;
    pr.ast.analyze(
        pr.root.unwrap(),
        &mut |id| {
            if matches!(pr.ast.get(id), shellcheck_core::ast::Token::T_Literal(l) if l == s) {
                found.get_or_insert(id);
            }
        },
        &mut |_| {},
    );
    found.unwrap_or_else(|| panic!("no literal {s:?}"))
}

#[test]
fn token_positions_use_the_sourced_file_name() {
    let pr = parse_with_lib("#!/bin/bash\nsource lib\necho mainword", "echo libword");
    let (s, e) = pr.token_positions(find_literal(&pr, "libword")).unwrap();
    assert_eq!((s.file.as_str(), s.line, s.column, e.file.as_str(), e.line, e.column), ("lib", 1, 6, "lib", 1, 13));
    let (s, e) = pr.token_positions(find_literal(&pr, "mainword")).unwrap();
    assert_eq!((s.file.as_str(), s.line, s.column, e.file.as_str(), e.column), ("main", 3, 6, "main", 14));
    assert!(pr.token_positions(shellcheck_core::ast::Id(pr.ast.len())).is_none());
}

#[test]
fn fix_replacements_get_the_comment_file() {
    use shellcheck_core::interface::{Fix, Replacement, TokenComment, new_replacement};
    let pr = parse_with_lib("#!/bin/bash\nsource lib\necho mainword", "echo libword");
    let rep = |file: &str, line, column| Replacement {
        start_pos: Position { file: file.into(), line, column },
        end_pos: Position { file: file.into(), line, column: column + 1 },
        ..new_replacement()
    };
    let tc = |id, reps: Vec<Replacement>| TokenComment {
        id,
        comment: Comment { severity: Severity::InfoC, code: 2086, message: "m".into() },
        fix: Some(Fix { replacements: reps }),
    };
    let files_of = |pc: PositionedComment| -> Vec<(String, String)> {
        pc.fix.unwrap().replacements.into_iter().map(|r| (r.start_pos.file, r.end_pos.file)).collect()
    };
    // A token in the sourced file: empty files become "lib"; a set file is kept.
    let pc = shellcheck_checker::token_to_position(
        &pr,
        tc(find_literal(&pr, "libword"), vec![rep("", 1, 6), rep("other", 1, 13)]),
    );
    assert_eq!(pc.start_pos.file, "lib");
    assert_eq!(files_of(pc), vec![("lib".into(), "lib".into()), ("other".into(), "other".into())]);
    // A main-file token.
    let pc = shellcheck_checker::token_to_position(&pr, tc(find_literal(&pr, "mainword"), vec![rep("", 3, 6)]));
    assert_eq!(files_of(pc), vec![("main".into(), "main".into())]);
}

#[test]
fn include_exclude_with_parser_codes() {
    let run = |included: Option<Vec<Code>>, excluded: Vec<Code>| {
        get_errors(
            &mocked_system_interface(vec![]),
            CheckSpec { included_warnings: included, excluded_warnings: excluded, ..spec("#!/bin/sh\necho \"$12\"") },
        )
    };
    assert_eq!(run(None, vec![]), vec![1037]);
    assert_eq!(run(Some(vec![1037]), vec![]), vec![1037]);
    assert_eq!(run(Some(vec![1000]), vec![]), none());
    assert_eq!(run(None, vec![1037]), none());
    // An include list wins over the exclude list, as in Haskell.
    assert_eq!(run(Some(vec![1037]), vec![1037]), vec![1037]);
}

#[test]
fn glob_classes_with_regex_set_syntax_do_not_panic() {
    assert!(matches_glob("foo[[].sh", "foo[.sh"));
    assert!(!matches_glob("foo[[].sh", "foox.sh"));
    assert!(matches_glob("a[&&b]", "a&"));
    assert!(matches_glob("a[~~b]", "a~"));
    assert!(matches_glob("a[--b]", "a-"));
    assert!(matches_glob("a[-b]", "a-"));
    assert!(matches_glob("a[b-]", "a-"));
    assert!(matches_glob("a[a-c]", "ab"));
    assert!(!matches_glob("a[a-c]", "a-"));
    assert!(matches_glob("a[\\]", "a\\"));
    assert!(!matches_glob("a[]", "a"));
    assert!(matches_glob("a[!]", "ax"));
    // A section header with such a class goes through the whole parse.
    assert_eq!(parse_editor_config("[foo[[].sh]\nshellcheck.shell=sh\n", "foo[.sh"), "\nshell=sh\n");
}
