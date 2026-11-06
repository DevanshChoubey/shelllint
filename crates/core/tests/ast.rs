// SPDX-License-Identifier: GPL-3.0-only
// Tests for the port of ShellCheck src/ShellCheck/AST.hs (e11f964)
#![allow(non_camel_case_types)]

use shellcheck_core::ast::{Ast, CaseType, Id, Pos, Token};
use shellcheck_core::interface::Severity;
use shellcheck_core::regex::{matches, mk_regex};

fn p() -> Pos {
    Pos { line: 1, column: 1 }
}

fn leaf(ast: &mut Ast, t: Token) -> Id {
    let id = ast.alloc(p(), p());
    ast.set(id, t);
    id
}

#[test]
fn alloc_set_get() {
    let mut ast = Ast::new();
    let a = ast.alloc(p(), p());
    let w = ast.alloc(p(), Pos { line: 1, column: 2 });
    assert_eq!(a, Id(0));
    assert_eq!(w, Id(1));
    ast.set(a, Token::T_Literal("a".to_string()));
    ast.set(w, Token::T_NormalWord(vec![a]));
    assert_eq!(ast.get(a), &Token::T_Literal("a".to_string()));
    assert_eq!(ast.get(w), &Token::T_NormalWord(vec![a]));
    assert_eq!(ast.span(w), (p(), Pos { line: 1, column: 2 }));
    assert_eq!(ast.len(), 2);
}

#[test]
fn children_order_matches_traversable() {
    let mut ast = Ast::new();
    let v1 = leaf(&mut ast, Token::T_Literal("v".into()));
    let w1 = leaf(&mut ast, Token::T_Literal("w1".into()));
    let w2 = leaf(&mut ast, Token::T_Literal("w2".into()));
    let sc = leaf(&mut ast, Token::T_SimpleCommand(vec![v1], vec![w1, w2]));
    assert_eq!(ast.children(sc).collect::<Vec<_>>(), vec![v1, w1, w2]);

    let c = leaf(&mut ast, Token::T_Literal("c".into()));
    let b = leaf(&mut ast, Token::T_Literal("b".into()));
    let e = leaf(&mut ast, Token::T_Literal("e".into()));
    let ife = leaf(&mut ast, Token::T_IfExpression(vec![(vec![c], vec![b])], vec![e]));
    assert_eq!(ast.children(ife).collect::<Vec<_>>(), vec![c, b, e]);

    let w = leaf(&mut ast, Token::T_Literal("w".into()));
    let pt = leaf(&mut ast, Token::T_Literal("p".into()));
    let body = leaf(&mut ast, Token::T_Literal("body".into()));
    let case = leaf(
        &mut ast,
        Token::T_CaseExpression(w, vec![(CaseType::CaseBreak, vec![pt], vec![body])]),
    );
    assert_eq!(ast.children(case).collect::<Vec<_>>(), vec![w, pt, body]);

    // The coprocess name is a `Maybe Token`, not a `t`, so Traversable skips it.
    let name = leaf(&mut ast, Token::T_Literal("name".into()));
    let cop = leaf(&mut ast, Token::T_CoProc(Some(name), body));
    assert_eq!(ast.children(cop).collect::<Vec<_>>(), vec![body]);
}

#[test]
fn analyze_pre_post_order() {
    let mut ast = Ast::new();
    let a = leaf(&mut ast, Token::T_Literal("a".into()));
    let b = leaf(&mut ast, Token::T_Literal("b".into()));
    let root = leaf(&mut ast, Token::T_NormalWord(vec![a, b]));
    let log = std::cell::RefCell::new(Vec::new());
    ast.analyze(
        root,
        &mut |id| log.borrow_mut().push(("pre", id.0)),
        &mut |id| log.borrow_mut().push(("post", id.0)),
    );
    assert_eq!(
        log.into_inner(),
        vec![("pre", 2), ("pre", 0), ("post", 0), ("pre", 1), ("post", 1), ("post", 2)]
    );
}

#[test]
fn parents_computed_from_root() {
    let mut ast = Ast::new();
    let a = leaf(&mut ast, Token::T_Literal("a".into()));
    let root = leaf(&mut ast, Token::T_NormalWord(vec![a]));
    ast.compute_parents(root);
    assert_eq!(ast.parent(a), Some(root));
    assert_eq!(ast.parent(root), None);
}

#[test]
fn structural_eq_ignores_ids() {
    let mut ast = Ast::new();
    let x1 = leaf(&mut ast, Token::T_Literal("x".into()));
    let w1 = leaf(&mut ast, Token::T_NormalWord(vec![x1]));
    let x2 = leaf(&mut ast, Token::T_Literal("x".into()));
    let w2 = leaf(&mut ast, Token::T_NormalWord(vec![x2]));
    let y = leaf(&mut ast, Token::T_Literal("y".into()));
    let w3 = leaf(&mut ast, Token::T_NormalWord(vec![y]));
    assert!(ast.structurally_eq(w1, w2));
    assert!(!ast.structurally_eq(w1, w3));

    // The non-traversed coprocess name is still compared structurally.
    let b = leaf(&mut ast, Token::T_Literal("b".into()));
    let c1 = leaf(&mut ast, Token::T_CoProc(Some(x1), b));
    let c2 = leaf(&mut ast, Token::T_CoProc(Some(x2), b));
    let c3 = leaf(&mut ast, Token::T_CoProc(Some(y), b));
    assert!(ast.structurally_eq(c1, c2));
    assert!(!ast.structurally_eq(c1, c3));
}

#[test]
fn truncate_drops_later_ids() {
    let mut ast = Ast::new();
    for _ in 0..5 {
        ast.alloc(p(), p());
    }
    ast.truncate(2);
    assert_eq!(ast.len(), 2);
    assert_eq!(ast.alloc(p(), p()), Id(2));
}

#[test]
fn regex_posix_classes() {
    assert!(matches("a b", &mk_regex("[[:space:]]")));
    assert!(!matches("ab", &mk_regex("[[:space:]]")));
}

#[test]
fn severity_order() {
    assert!(Severity::ErrorC < Severity::StyleC);
    assert!(Severity::ErrorC < Severity::WarningC);
    assert!(Severity::WarningC < Severity::InfoC);
    assert!(Severity::InfoC < Severity::StyleC);
}
