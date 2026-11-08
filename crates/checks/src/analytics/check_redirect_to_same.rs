// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkRedirectToSame`, lines 868-920 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params, get_closest_command, make_comment};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::{get_command_basename, get_path, is_assignment, oversimplify};
use shelllint_core::interface::Severity;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkRedirectToSame",
    registry: "node",
    order: 48,
    optional: None,
    kind: CheckKind::Node(check_redirect_to_same),
};

fn check_redirect_to_same(p: &Params<'_>, s: Id, c: &mut Comments) {
    let Token::T_Pipeline(_, list) = p.ast.get(s) else { return };
    let ast = p.ast;

    let note = |x: Id| {
        make_comment(
            Severity::InfoC,
            x,
            2094,
            "Make sure not to read and write the same file in the same pipeline.",
        )
    };

    // getRedirs
    let get_redirs = |t: Id| -> Vec<Id> {
        if let Token::T_FdRedirect(_, io) = ast.get(t) {
            if let Token::T_IoFile(op, file) = ast.get(*io) {
                return match ast.get(*op) {
                    Token::T_Greater | Token::T_Less | Token::T_DGREAT => vec![*file],
                    _ => vec![],
                };
            }
        }
        vec![]
    };
    // getAllRedirs
    let mut all_redirs: Vec<Id> = Vec::new();
    for &t in list {
        if let Token::T_Redirecting(ls, _) = ast.get(t) {
            for &l in ls {
                all_redirs.extend(get_redirs(l));
            }
        }
    }

    let special = |x: Id| oversimplify(ast, x).concat().starts_with("/dev/");
    let io_op = |t: Id| -> Option<Id> {
        let path = get_path(ast, t);
        match path.get(1) {
            Some(&parent) => match ast.get(parent) {
                Token::T_IoFile(op, _) => Some(*op),
                _ => None,
            },
            None => None,
        }
    };
    let is_input = |t: Id| io_op(t).is_some_and(|op| matches!(ast.get(op), Token::T_Less));
    let is_output = |t: Id| {
        io_op(t).is_some_and(|op| matches!(ast.get(op), Token::T_Greater | Token::T_DGREAT))
    };
    let is_harmless_command = |arg: Id| -> bool {
        let Some(cmd) = get_closest_command(ast, arg) else { return false };
        let Some(name) = get_command_basename(ast, cmd) else { return false };
        ["echo", "mapfile", "printf", "sponge"].contains(&name.as_str())
    };
    let contains_assignment = |arg: Id| -> bool {
        let Some(cmd) = get_closest_command(ast, arg) else { return false };
        is_assignment(ast, cmd)
    };

    for &l in list {
        for &x in &all_redirs {
            // doAnalysis (checkOccurrences x) l
            let mut visit = |u: Id| {
                let t = x;
                let (Token::T_NormalWord(xs), Token::T_NormalWord(ys)) = (ast.get(t), ast.get(u))
                else {
                    return;
                };
                let except_id = t;
                let new_id = u;
                if except_id != new_id
                    && xs.len() == ys.len()
                    && xs.iter().zip(ys.iter()).all(|(&a, &b)| ast.structurally_eq(a, b))
                    && !(is_input(t) && is_input(u))
                    && !(is_output(t) && is_output(u))
                    && !special(t)
                    && ![t, u].iter().any(|&a| is_harmless_command(a))
                    && !contains_assignment(u)
                {
                    c.add_comment(note(new_id));
                    c.add_comment(note(except_id));
                }
            };
            ast.analyze(l, &mut visit, &mut |_| {});
        }
    }
}
