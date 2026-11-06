// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkMultipleAppends`, lines 3175-3193 (e11f964)

use shellcheck_analyzer::analyzerlib::{Comments, Params};
use shellcheck_analyzer::api::{CheckEntry, CheckKind};
use shellcheck_core::ast::{Id, Token};
use shellcheck_core::astlib::get_command_sequences;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkMultipleAppends",
    registry: "node",
    order: 59,
    optional: None,
    kind: CheckKind::Node(check_multiple_appends),
};

/// `groupWith f = groupBy ((==) `on` f)`, specialised to a keyed list with a
/// caller-supplied equality on keys. Each group is compared to its first element.
fn group_with<T: Copy, K>(l: &[T], f: impl Fn(T) -> K, eq: impl Fn(&K, &K) -> bool) -> Vec<Vec<T>> {
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for &x in l {
        let k = f(x);
        match groups.last_mut() {
            Some((k0, g)) if eq(k0, &k) => g.push(x),
            _ => groups.push((k, vec![x])),
        }
    }
    groups.into_iter().map(|(_, g)| g).collect()
}

fn check_multiple_appends(p: &Params<'_>, t: Id, c: &mut Comments) {
    for list in get_command_sequences(p.ast, t) {
        check_list(p, &list, c);
    }
}

fn check_list(p: &Params<'_>, list: &[Id], c: &mut Comments) {
    let targets: Vec<Option<(Id, Id)>> = list.iter().map(|&x| get_target(p, x)).collect();
    let eq = |a: &Option<(Id, Id)>, b: &Option<(Id, Id)>| match (a, b) {
        (None, None) => true,
        (Some((f1, _)), Some((f2, _))) => p.ast.structurally_eq(*f1, *f2),
        _ => false,
    };
    for group in group_with(&targets, |x| x, |a, b| eq(a, b)) {
        // checkGroup (Just (_,id):_:_:_)
        if group.len() >= 3 {
            if let Some((_, id)) = group[0] {
                c.style(id, 2129, "Consider using { cmd1; cmd2; } >> file instead of individual redirects.");
            }
        }
    }
}

fn get_target(p: &Params<'_>, t: Id) -> Option<(Id, Id)> {
    match p.ast.get(t) {
        Token::T_Annotation(_, inner) => get_target(p, *inner),
        Token::T_Pipeline(_, args) if !args.is_empty() => get_target(p, *args.last().unwrap()),
        Token::T_Redirecting(list, _) => {
            let file = list.iter().find_map(|&r| get_append(p, r))?;
            Some((file, t))
        }
        _ => None,
    }
}

fn get_append(p: &Params<'_>, t: Id) -> Option<Id> {
    let Token::T_FdRedirect(_, op) = p.ast.get(t) else { return None };
    let Token::T_IoFile(op, f) = p.ast.get(*op) else { return None };
    match p.ast.get(*op) {
        Token::T_DGREAT => Some(*f),
        _ => None,
    }
}
