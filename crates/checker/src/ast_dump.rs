// SPDX-License-Identifier: GPL-3.0-or-later
// `--dump-ast` (an extension beyond ShellCheck, issue #2696): print the parsed
// AST of a script as compact JSON.
//
// DIVERGENCE: upstream ShellCheck has no AST dump; the shape below is this
// port's own and is derived from the parser's real token types.
//
// Document shape (one per file):
//
//   {"file": NAME, "parsed": true,  "ast": NODE}
//   {"file": NAME, "parsed": false, "ast": null, "errors": [ERROR...]}
//
//   NODE  = {"type": "T_Foo", "id": N,
//            "start": {"line": L, "column": C}, "end": {"line": L, "column": C},
//            "value": "..."?, "children": [NODE...]}
//   ERROR = {"line": L, "column": C, "code": N, "message": "..."}
//
// `type` is the name of the `Token` variant (`shelllint_core::ast::Token`).
// `id` is the arena id (`Id`), unique within a file. Lines and columns are
// 1-based, tabs count as 8, and `end` is the position the parser stopped at
// (just past the token). `value` is present for tokens that carry a string
// (a literal's text, an operator, a variable or function name, ...).
// `children` lists the child tokens in the parser's traversal order and is
// always present (empty for leaves). Tokens the parser did not attach to the
// tree are not printed.
//
// The tree is written straight to the output while walking the arena with an
// explicit stack, so no second copy of the tree is built and deep nesting
// cannot overflow the stack.

use std::fmt::{self, Write as FmtWrite};
use std::io::{self, Write};

use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::interface::{CheckSpec, ParseSpec, PositionedComment, SystemInterface, new_parse_spec};
use shelllint_parser::{ParseResult, parse_script};

/// Parse `spec.script` and write its JSON document to `out`. Returns whether
/// the script parsed. Parse failures are described inside the document.
pub fn dump_ast(sys: &dyn SystemInterface, mut spec: CheckSpec, out: &mut dyn Write) -> io::Result<bool> {
    let filename = spec.filename.clone();
    let script = std::mem::take(&mut spec.script);
    let result = super::run_on_check_stack(|| {
        parse_script(
            sys,
            &ParseSpec {
                filename: spec.filename.clone(),
                script,
                check_sourced: spec.check_sourced,
                ignore_rc: spec.ignore_rc,
                shell_type_override: spec.shell_type_override,
                ..new_parse_spec()
            },
        )
    });
    write_document(&filename, &result, out)
}

fn write_document(filename: &str, result: &ParseResult, out: &mut dyn Write) -> io::Result<bool> {
    out.write_all(b"{\"file\":")?;
    write_str(out, filename)?;
    match result.root {
        Some(root) => {
            out.write_all(b",\"parsed\":true,\"ast\":")?;
            write_tree(&result.ast, root, out)?;
            out.write_all(b"}")?;
            Ok(true)
        }
        None => {
            out.write_all(b",\"parsed\":false,\"ast\":null,\"errors\":[")?;
            for (i, pc) in result.comments.iter().enumerate() {
                if i > 0 {
                    out.write_all(b",")?;
                }
                write_error(out, pc)?;
            }
            out.write_all(b"]}")?;
            Ok(false)
        }
    }
}

fn write_error(out: &mut dyn Write, pc: &PositionedComment) -> io::Result<()> {
    write!(
        out,
        "{{\"line\":{},\"column\":{},\"code\":{},\"message\":",
        pc.start_pos.line, pc.start_pos.column, pc.comment.code
    )?;
    write_str(out, &pc.comment.message)?;
    out.write_all(b"}")
}

fn write_tree(ast: &Ast, root: Id, out: &mut dyn Write) -> io::Result<()> {
    // Each frame is a node whose children are being written: its child ids
    // and how many are done.
    let mut stack: Vec<(Vec<Id>, usize)> = Vec::new();
    stack.push((open_node(ast, root, out)?, 0));
    while let Some(frame) = stack.last_mut() {
        if frame.1 < frame.0.len() {
            let child = frame.0[frame.1];
            if frame.1 > 0 {
                out.write_all(b",")?;
            }
            frame.1 += 1;
            let kids = open_node(ast, child, out)?;
            stack.push((kids, 0));
        } else {
            out.write_all(b"]}")?;
            stack.pop();
        }
    }
    Ok(())
}

/// Write a node up to and including `"children":[`; returns its child ids.
fn open_node(ast: &Ast, id: Id, out: &mut dyn Write) -> io::Result<Vec<Id>> {
    let tok = ast.get(id);
    let (s, e) = ast.span(id);
    write!(out, "{{\"type\":\"{}\",\"id\":{}", type_name(tok), id.0)?;
    write!(
        out,
        ",\"start\":{{\"line\":{},\"column\":{}}},\"end\":{{\"line\":{},\"column\":{}}}",
        s.line, s.column, e.line, e.column
    )?;
    if let Some(v) = value(tok) {
        out.write_all(b",\"value\":")?;
        write_str(out, v)?;
    }
    out.write_all(b",\"children\":[")?;
    Ok(tok.children())
}

/// The variant name, without formatting the payload: a `fmt::Write` sink
/// that stops the derived `Debug` as soon as the name ends.
fn type_name(tok: &Token) -> String {
    struct Name(String);
    impl FmtWrite for Name {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for c in s.chars() {
                if !(c.is_ascii_alphanumeric() || c == '_') {
                    return Err(fmt::Error);
                }
                self.0.push(c);
            }
            Ok(())
        }
    }
    let mut n = Name(String::new());
    let _ = write!(n, "{:?}", tok);
    n.0
}

/// The string payload of a token, if it has a meaningful one.
fn value(tok: &Token) -> Option<&str> {
    match tok {
        Token::TA_Binary(s, ..)
        | Token::TA_Assignment(s, ..)
        | Token::TA_Variable(s, _)
        | Token::TA_Unary(s, _)
        | Token::TC_And(_, s, ..)
        | Token::TC_Binary(_, s, ..)
        | Token::TC_Or(_, s, ..)
        | Token::TC_Unary(_, s, _)
        | Token::T_UnparsedIndex(_, s)
        | Token::T_Assignment(_, s, ..)
        | Token::T_DollarSingleQuoted(s)
        | Token::T_Extglob(s, _)
        | Token::T_FdRedirect(s, _)
        | Token::T_ForIn(s, ..)
        | Token::T_Function(_, _, s, _)
        | Token::T_Glob(s)
        | Token::T_HereDoc(_, _, s, _)
        | Token::T_IoDuplicate(_, s)
        | Token::T_Literal(s)
        | Token::T_ParamSubSpecialChar(s)
        | Token::T_ProcSub(s, _)
        | Token::T_SelectIn(s, ..)
        | Token::T_SingleQuoted(s)
        | Token::T_Pipe(s)
        | Token::T_BatsTest(s, _) => Some(s),
        _ => None,
    }
}

fn write_str(out: &mut dyn Write, s: &str) -> io::Result<()> {
    out.write_all(b"\"")?;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        let esc: &str = match c {
            '"' => "\\\"",
            '\\' => "\\\\",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            c if (c as u32) < 0x20 => {
                out.write_all(&s.as_bytes()[start..i])?;
                write!(out, "\\u{:04x}", c as u32)?;
                start = i + 1;
                continue;
            }
            _ => continue,
        };
        out.write_all(&s.as_bytes()[start..i])?;
        out.write_all(esc.as_bytes())?;
        start = i + c.len_utf8();
    }
    out.write_all(&s.as_bytes()[start..])?;
    out.write_all(b"\"")
}

/// The document for a file that could not be read.
pub fn dump_read_failure(filename: &str, message: &str, out: &mut dyn Write) -> io::Result<()> {
    out.write_all(b"{\"file\":")?;
    write_str(out, filename)?;
    out.write_all(b",\"parsed\":false,\"ast\":null,\"errors\":[{\"message\":")?;
    write_str(out, message)?;
    out.write_all(b"}]}")
}
