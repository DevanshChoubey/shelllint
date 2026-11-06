// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the prop_ tests in ShellCheck src/ShellCheck/ASTLib.hs (e11f964)
//
// None of the 61 ASTLib unit tests parse a script: they are either on
// hand-built single-token trees or on plain strings, so none needs
// `#[ignore = "needs parser"]`.
use shellcheck_analyzer::astlib::*;
use shellcheck_core::ast::{Ast, Id, Pos, Token};

fn dsq(s: &str) -> (Ast, Id) {
    let mut ast = Ast::new();
    let p = Pos { line: 1, column: 1 };
    let id = ast.alloc(p, p);
    ast.set(id, Token::T_DollarSingleQuoted(s.to_string()));
    (ast, id)
}

fn dsq_lit(s: &str) -> Option<String> {
    let (ast, id) = dsq(s);
    get_literal_string(&ast, id)
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn get_literal_string1() {
    assert_eq!(dsq_lit("\\x01"), some("\x01"));
}

#[test]
fn get_literal_string2() {
    assert_eq!(dsq_lit("\\xyz"), some("\\xyz"));
}

#[test]
fn get_literal_string3() {
    assert_eq!(dsq_lit("\\x1"), some("\x01"));
}

#[test]
fn get_literal_string4() {
    assert_eq!(dsq_lit("\\x1y"), some("\x01y"));
}

#[test]
fn get_literal_string5() {
    assert_eq!(dsq_lit("\\xy"), some("\\xy"));
}

#[test]
fn get_literal_string6() {
    assert_eq!(dsq_lit("\\x"), some("\\x"));
}

#[test]
fn get_literal_string7() {
    assert_eq!(dsq_lit("\\1x"), some("\x01x"));
}

#[test]
fn get_literal_string8() {
    assert_eq!(dsq_lit("\\12x"), some("\nx"));
}

#[test]
fn get_literal_string9() {
    assert_eq!(dsq_lit("\\123x"), some("Sx"));
}

#[test]
fn get_literal_string10() {
    assert_eq!(dsq_lit("\\1234"), some("\u{53}4"));
}

#[test]
fn get_literal_string11() {
    assert_eq!(dsq_lit("\\1"), some("\x01"));
}

#[test]
fn get_literal_string12() {
    assert_eq!(dsq_lit("\\12"), some("\n"));
}

#[test]
fn get_literal_string13() {
    assert_eq!(dsq_lit("\\123"), some("S"));
}

#[test]
fn get_literal_string14() {
    assert_eq!(dsq_lit("\\e[1mfoo\\E[0mbar"), some("\x1b[1mfoo\x1b[0mbar"));
}

#[test]
fn get_literal_string15() {
    assert_eq!(dsq_lit("\\?"), some("?"));
}

#[test]
fn get_literal_string16() {
    assert_eq!(dsq_lit("\\u9"), some("\t"));
}

#[test]
fn get_literal_string17() {
    assert_eq!(dsq_lit("\\u2F"), some("/"));
}

#[test]
fn get_literal_string18() {
    assert_eq!(dsq_lit("\\u100"), some("Ā"));
}

#[test]
fn get_literal_string19() {
    assert_eq!(dsq_lit("\\u1d00"), some("ᴀ"));
}

#[test]
fn get_literal_string20() {
    assert_eq!(dsq_lit("\\u1D56C"), some("ᵖC"));
}

#[test]
fn get_literal_string21() {
    assert_eq!(dsq_lit("\\U9z"), some("\tz"));
}

#[test]
fn get_literal_string22() {
    assert_eq!(dsq_lit("\\u1d00."), some("ᴀ."));
}

#[test]
fn get_literal_string23() {
    assert_eq!(dsq_lit("\\U1D56C"), some("𝕬"));
}

#[test]
fn executable_from_shebang1() {
    assert_eq!(executable_from_shebang("/bin/sh"), "sh");
}

#[test]
fn executable_from_shebang2() {
    assert_eq!(executable_from_shebang("/bin/bash"), "bash");
}

#[test]
fn executable_from_shebang3() {
    assert_eq!(executable_from_shebang("/usr/bin/env ksh"), "ksh");
}

#[test]
fn executable_from_shebang4() {
    assert_eq!(executable_from_shebang("/usr/bin/env -S foo=bar bash -x"), "bash");
}

#[test]
fn executable_from_shebang5() {
    assert_eq!(executable_from_shebang("/usr/bin/env --split-string=bash -x"), "bash");
}

#[test]
fn executable_from_shebang6() {
    assert_eq!(executable_from_shebang("/usr/bin/env --split-string=foo=bar bash -x"), "bash");
}

#[test]
fn executable_from_shebang7() {
    assert_eq!(executable_from_shebang("/usr/bin/env --split-string bash -x"), "bash");
}

#[test]
fn executable_from_shebang8() {
    assert_eq!(executable_from_shebang("/usr/bin/env --split-string foo=bar bash -x"), "bash");
}

#[test]
fn executable_from_shebang9() {
    assert_eq!(executable_from_shebang("/usr/bin/env foo=bar dash"), "dash");
}

#[test]
fn executable_from_shebang10() {
    assert_eq!(executable_from_shebang("/bin/busybox sh"), "busybox sh");
}

#[test]
fn executable_from_shebang11() {
    assert_eq!(executable_from_shebang("/bin/busybox ash"), "busybox ash");
}

#[test]
fn is_variable_name1() {
    assert!(is_variable_name("_fo123"));
}

#[test]
fn is_variable_name2() {
    assert!(!is_variable_name("4"));
}

#[test]
fn is_variable_name3() {
    assert!(!is_variable_name("test: "));
}

#[test]
fn get_braced_reference1() {
    assert_eq!(get_braced_reference("foo"), "foo");
}

#[test]
fn get_braced_reference2() {
    assert_eq!(get_braced_reference("#foo"), "foo");
}

#[test]
fn get_braced_reference3() {
    assert_eq!(get_braced_reference("#"), "#");
}

#[test]
fn get_braced_reference4() {
    assert_eq!(get_braced_reference("##"), "#");
}

#[test]
fn get_braced_reference5() {
    assert_eq!(get_braced_reference("#!"), "!");
}

#[test]
fn get_braced_reference6() {
    assert_eq!(get_braced_reference("!#"), "#");
}

#[test]
fn get_braced_reference7() {
    assert_eq!(get_braced_reference("!foo#?"), "foo");
}

#[test]
fn get_braced_reference8() {
    assert_eq!(get_braced_reference("foo-bar"), "foo");
}

#[test]
fn get_braced_reference9() {
    assert_eq!(get_braced_reference("foo:-bar"), "foo");
}

#[test]
fn get_braced_reference10() {
    assert_eq!(get_braced_reference("foo: -1"), "foo");
}

#[test]
fn get_braced_reference11() {
    assert_eq!(get_braced_reference("!os*"), "");
}

#[test]
fn get_braced_reference11b() {
    assert_eq!(get_braced_reference("!os@"), "");
}

#[test]
fn get_braced_reference12() {
    assert_eq!(get_braced_reference("!os?bar**"), "");
}

#[test]
fn get_braced_reference13() {
    assert_eq!(get_braced_reference("foo[bar]"), "foo");
}

#[test]
fn get_braced_modifier1() {
    assert_eq!(get_braced_modifier("foo:bar:baz"), ":bar:baz");
}

#[test]
fn get_braced_modifier2() {
    assert_eq!(get_braced_modifier("!var:-foo"), ":-foo");
}

#[test]
fn get_braced_modifier3() {
    assert_eq!(get_braced_modifier("foo[bar]"), "[bar]");
}

#[test]
fn get_braced_modifier4() {
    assert_eq!(get_braced_modifier("foo[@]@Q"), "[@]@Q");
}

#[test]
fn get_braced_modifier5() {
    assert_eq!(get_braced_modifier("@@Q"), "@Q");
}

#[test]
fn get_index_references1() {
    assert_eq!(get_index_references("var[x+y+1]"), vec!["x", "y"]);
}

#[test]
fn get_offset_references1() {
    assert_eq!(get_offset_references(":bar"), vec!["bar"]);
}

#[test]
fn get_offset_references2() {
    assert_eq!(get_offset_references(":bar:baz"), vec!["bar", "baz"]);
}

#[test]
fn get_offset_references3() {
    assert_eq!(get_offset_references("[foo]:bar"), vec!["bar"]);
}

#[test]
fn get_offset_references4() {
    assert_eq!(get_offset_references("[foo]:bar:baz"), vec!["bar", "baz"]);
}

// Extra tests (not from ASTLib.hs) on hand-built trees.
mod extra {
    use super::*;

    const P: Pos = Pos { line: 1, column: 1 };

    fn mk(ast: &mut Ast, t: Token) -> Id {
        let id = ast.alloc(P, P);
        ast.set(id, t);
        id
    }
    fn word(ast: &mut Ast, s: &str) -> Id {
        let l = mk(ast, Token::T_Literal(s.to_string()));
        mk(ast, Token::T_NormalWord(vec![l]))
    }
    fn cmd(ast: &mut Ast, words: &[&str]) -> Id {
        let ids: Vec<Id> = words.iter().map(|w| word(ast, w)).collect();
        mk(ast, Token::T_SimpleCommand(vec![], ids))
    }

    #[test]
    fn flags_and_command_name() {
        let mut ast = Ast::new();
        let c = cmd(&mut ast, &["ls", "-la", "--color=auto", "x", "--", "-z"]);
        let flags: Vec<String> = get_all_flags(&ast, c).into_iter().map(|(_, s)| s).collect();
        assert_eq!(flags, vec!["l", "a", "color", "", "", ""]);
        assert!(has_flag(&ast, c, "a"));
        assert_eq!(get_command_name(&ast, c), some("ls"));
        let e = cmd(&mut ast, &["exec", "-c", "ls", "-l"]);
        assert_eq!(get_command_name(&ast, e), some("ls"));
        let b = cmd(&mut ast, &["command", "-v", "ls"]);
        assert_eq!(get_command_name(&ast, b), some("command"));
    }

    #[test]
    fn gnu_opts() {
        let mut ast = Ast::new();
        let args: Vec<Id> = ["-re", "-d", ":", "bar"].iter().map(|w| word(&mut ast, w)).collect();
        let opts = get_gnu_opts(&ast, "erd:", &args).unwrap();
        let names: Vec<&str> = opts.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["r", "e", "d", ""]);
        assert_eq!(opts[2].1, (args[1], args[2]));
        assert!(get_gnu_opts(&ast, "er", &args).is_none());
    }

    #[test]
    fn brace_expansion() {
        let mut ast = Ast::new();
        let pre = mk(&mut ast, Token::T_Literal("a".into()));
        let w1 = word(&mut ast, "x");
        let w2 = word(&mut ast, "y");
        let be = mk(&mut ast, Token::T_BraceExpansion(vec![w1, w2]));
        let nw = mk(&mut ast, Token::T_NormalWord(vec![pre, be]));
        let out = brace_expand(&mut ast, nw);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn pseudo_globs() {
        let v = [PseudoGlob::PGChar('f'), PseudoGlob::PGMany, PseudoGlob::PGAny, PseudoGlob::PGMany];
        assert_eq!(simplify_pseudo_glob(&v), vec![PseudoGlob::PGChar('f'), PseudoGlob::PGAny, PseudoGlob::PGMany]);
        let a = [PseudoGlob::PGChar('a')];
        let b = [PseudoGlob::PGChar('b')];
        assert!(!pseudo_globs_can_overlap(&a, &b));
        assert!(pseudo_globs_can_overlap(&a, &[PseudoGlob::PGMany]));
        assert!(pseudo_glob_is_superset_of(&[PseudoGlob::PGMany], &a));
        assert!(!pseudo_glob_is_superset_of(&a, &[PseudoGlob::PGMany]));
    }

    #[test]
    fn escape_message() {
        assert_eq!(escape_for_message("a\\b\n\x01\u{1F600}\u{e9}"), "a\\\\b\\n\\x01\\U1F600\u{e9}");
    }

    #[test]
    fn path_to_root() {
        let mut ast = Ast::new();
        let c = cmd(&mut ast, &["ls"]);
        let r = mk(&mut ast, Token::T_Redirecting(vec![], c));
        ast.compute_parents(r);
        assert_eq!(get_path(&ast, c), vec![c, r]);
    }
}
