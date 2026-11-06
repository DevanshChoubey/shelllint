// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkPipePitfalls`, lines 600-656 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CHECKS, CheckEntry, CheckKind};
use shellcheck_core::ast::Id;
use shellcheck_core::astlib::{get_all_flags, get_command, get_command_token_or_this, oversimplify};
use shellcheck_core::prelude::head_or_default;

#[linkme::distributed_slice(CHECKS)]
static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkPipePitfalls",
    registry: "node",
    order: 0,
    optional: None,
    kind: CheckKind::Node(check_pipe_pitfalls),
};

fn check_pipe_pitfalls(p: &Params<'_>, t: Id, c: &mut Comments) {
    let shellcheck_core::ast::Token::T_Pipeline(_, commands) = p.ast.get(t) else { return };
    let commands: Vec<Id> = commands.clone();

    // for l f
    let for_ = |l: &[&str], f: &mut dyn FnMut(&[Id])| -> bool {
        let heads: Vec<String> = commands
            .iter()
            .map(|&x| head_or_default(String::new(), &oversimplify(p.ast, x)))
            .collect();
        let indices = index_of_sublists(l, &heads);
        for &n in &indices {
            f(&commands[n..n + l.len()]);
        }
        !indices.is_empty()
    };

    let has_short_parameter = |ch: char, args: &[String]| args.iter().any(|x| x.starts_with('-') && x.contains(ch));
    let has_parameter = |s: &str, args: &[String]| args.iter().any(|x| x.trim_start_matches('-').starts_with(s));
    let flags_of = |x: Id| -> Vec<String> {
        get_command(p.ast, x)
            .map(|cmd| get_all_flags(p.ast, cmd).into_iter().map(|(_, s)| s).collect())
            .unwrap_or_default()
    };

    for_(&["find", "xargs"], &mut |cs| {
        let (find, xargs) = (cs[0], cs[1]);
        let mut args = oversimplify(p.ast, xargs);
        args.extend(oversimplify(p.ast, find));
        let any = has_short_parameter('0', &args)
            || has_parameter("null", &args)
            || has_parameter("print0", &args)
            || has_parameter("printf", &args);
        if !any {
            c.warn(
                find,
                2038,
                "Use 'find .. -print0 | xargs -0 ..' or 'find .. -exec .. +' to allow non-alphanumeric filenames.",
            );
        }
    });

    for_(&["ps", "grep"], &mut |cs| {
        let ps = cs[0];
        let ps_flags = flags_of(ps);
        // There are many ways to specify a pid: 1, -1, p 1, wup 1, -q 1, -p 1, --pid 1.
        // For simplicity we only deal with the most canonical looking flags:
        if !ps_flags.iter().any(|f| ["p", "pid", "q", "quick-pid"].contains(&f.as_str())) {
            c.info(ps, 2009, "Consider using pgrep instead of grepping ps output.");
        }
    });

    for_(&["grep", "wc"], &mut |cs| {
        let (grep, wc) = (cs[0], cs[1]);
        let flags_grep = flags_of(grep);
        let flags_wc = flags_of(wc);
        let g = [
            "l", "files-with-matches", "L", "files-without-matches", "o", "only-matching", "r", "R",
            "recursive", "A", "after-context", "B", "before-context",
        ];
        let w = ["m", "chars", "w", "words", "c", "bytes", "L", "max-line-length"];
        if !(flags_grep.iter().any(|f| g.contains(&f.as_str()))
            || flags_wc.iter().any(|f| w.contains(&f.as_str()))
            || flags_wc.is_empty())
        {
            c.style(grep, 2126, "Consider using 'grep -c' instead of 'grep|wc -l'.");
        }
    });

    // for' l f = for l (first f)
    let first_id = |cs: &[Id]| get_command_token_or_this(p.ast, cs[0]);
    let did_ls1 = for_(&["ls", "grep"], &mut |cs| {
        c.warn(
            first_id(cs),
            2010,
            "Don't use ls | grep. Use a glob or a for loop with a condition to allow non-alphanumeric filenames.",
        );
    });
    let did_ls2 = for_(&["ls", "xargs"], &mut |cs| {
        c.warn(
            first_id(cs),
            2011,
            "Use 'find .. -print0 | xargs -0 ..' or 'find .. -exec .. +' to allow non-alphanumeric filenames.",
        );
    });
    let did_ls = did_ls1 || did_ls2;
    if !did_ls {
        for_(&["ls", "?"], &mut |cs| {
            let ls = cs[0];
            if !has_short_parameter('N', &oversimplify(p.ast, ls)) {
                c.info(ls, 2012, "Use find instead of ls to better handle non-alphanumeric filenames.");
            }
        });
    }
}

fn index_of_sublists(sub: &[&str], list: &[String]) -> Vec<usize> {
    fn matches(sub: &[&str], a: &[String]) -> bool {
        match (sub.split_first(), a.split_first()) {
            (Some((&"?", r1)), Some((_, r2))) => matches(r1, r2),
            (Some((x1, r1)), Some((x2, r2))) if *x1 == x2.as_str() => matches(r1, r2),
            (None, _) => true,
            _ => false,
        }
    }
    (0..list.len()).filter(|&n| matches(sub, &list[n..])).collect()
}
