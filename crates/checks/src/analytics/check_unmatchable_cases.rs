// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Analytics.hs `checkUnmatchableCases`, lines 3687-3729 (e11f964)

use shelllint_analyzer::analyzerlib::{Comments, Params};
use shelllint_analyzer::api::{CheckEntry, CheckKind};
use shelllint_core::ast::{CaseType, Id, Token};
use shelllint_core::astlib::{
    PseudoGlob, is_constant, oversimplify, pseudo_glob_is_superset_of, pseudo_globs_can_overlap,
    word_to_exact_pseudo_glob, word_to_pseudo_glob,
};
use shelllint_core::interface::Shell;

pub(crate) static ENTRY: CheckEntry = CheckEntry {
    module: "Analytics.hs",
    name: "checkUnmatchableCases",
    registry: "node",
    order: 71,
    optional: None,
    kind: CheckKind::Node(check_unmatchable_cases),
};

/// DIVERGENCE: zsh patterns have operators the pseudo-glob model does not know:
/// `(a|b)` groups, `<1-9>` ranges, `#`/`##` repeats, `^` and `~` exclusion, and
/// `(#i)` flags. A pattern that uses any of them can match anything, so it is
/// modelled as `*` (and has no exact form), which keeps SC2195, SC2221 and
/// SC2222 from firing on it.
fn has_zsh_pattern_syntax(p: &Params<'_>, word: Id) -> bool {
    p.shell_type == Shell::Zsh && oversimplify(p.ast, word).concat().chars().any(|c| "<>()|#^~".contains(c))
}

fn pseudo_glob(p: &Params<'_>, word: Id) -> Vec<PseudoGlob> {
    if has_zsh_pattern_syntax(p, word) { vec![PseudoGlob::PGMany] } else { word_to_pseudo_glob(p.ast, word) }
}

fn exact_pseudo_glob(p: &Params<'_>, word: Id) -> Option<Vec<PseudoGlob>> {
    if has_zsh_pattern_syntax(p, word) { None } else { word_to_exact_pseudo_glob(p.ast, word) }
}

fn check_unmatchable_cases(p: &Params<'_>, t: Id, c: &mut Comments) {
    let Token::T_CaseExpression(word, list) = p.ast.get(t) else { return };
    let word = *word;
    // Check all patterns for whether they can ever match
    let allpatterns: Vec<Id> = list.iter().flat_map(|(_, pats, _)| pats.iter().copied()).collect();
    // Check only the non-fallthrough branches for shadowing
    let breakpatterns: Vec<Id> = list
        .iter()
        .filter(|(ty, _, _)| *ty == CaseType::CaseBreak)
        .flat_map(|(_, pats, _)| pats.iter().copied())
        .collect();

    if is_constant(p.ast, word) {
        c.warn(word, 2194, "This word is constant. Did you forget the $ on a variable?");
    } else {
        let target = pseudo_glob(p, word);
        for &candidate in &allpatterns {
            if !pseudo_globs_can_overlap(&target, &pseudo_glob(p, candidate)) {
                c.warn(
                    candidate,
                    2195,
                    "This pattern will never match the case statement's word. Double check them.",
                );
            }
        }
    }

    let exact_globs: Vec<_> =
        breakpatterns.iter().map(|&x| (x, exact_pseudo_glob(p, x))).collect();
    let fuzzy_globs: Vec<_> =
        breakpatterns.iter().map(|&x| (x, pseudo_glob(p, x))).collect();

    let pattern_context = |id: Id| -> String {
        let (start, _) = p.ast.span(id);
        format!(" on line {}.", start.line)
    };

    for (i, (glob, exact)) in exact_globs.iter().enumerate() {
        let Some(x) = exact else { continue };
        let rest = fuzzy_globs.get(i + 1..).unwrap_or(&[]);
        if let Some((first, _)) = rest.iter().find(|(_, pg)| pseudo_glob_is_superset_of(x, pg)) {
            c.warn(
                *glob,
                2221,
                format!("This pattern always overrides a later one{}", pattern_context(*first)),
            );
            c.warn(
                *first,
                2222,
                format!(
                    "This pattern never matches because of a previous pattern{}",
                    pattern_context(*glob)
                ),
            );
        }
    }
}
