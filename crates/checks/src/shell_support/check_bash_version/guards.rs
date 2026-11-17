// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint's own `--bash-version` guard recognition, no Haskell counterpart.
// A test on the bash version protects the code it guards: `if ((BASH_VERSINFO[0] >= 4)); then
// declare -A m; fi` runs only on bash 4 or later, so `declare -A` is not too new there.
//
// `floor` gives the least version that a guard proves for a node. A feature is reported only
// when neither the target nor that floor reaches the feature's release. Only the spellings
// below are guards. Any other condition is `Guard::Unknown`, which proves nothing:
// - operands: `BASH_VERSINFO[0]`, `BASH_VERSINFO` (element 0), `BASH_VERSINFO[1]` (minor), and
//   `${BASH_VERSION%%.*}` / `${BASH_VERSION:0:1}` (major). A bare name is read as arithmetic only
//   in `(( ))` and `[[ ]]`, so `[ BASH_VERSINFO -ge 4 ]` is not a guard, as in bash itself.
// - comparisons with a literal integer: `>= > == < <=` in `(( ))`, `== -eq -ge -gt -lt -le` in
//   `[[ ]]`, and `-eq -ge -gt -lt -le` in `[ ]` and `test`. The literal may be on either side.
// - `&&`, `||`, `!` and parentheses combine guards. A combined major and minor test such as
//   `BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3)` gives 4.3.
// Regions: the body of `if G; then`, each `elif`, the `else` of `if NOT_G`, the right side of
// `G && cmd` and `NOT_G || cmd`, and every later statement in a list after an early exit
// (`if NOT_G; then ...; exit|return; fi`, `NOT_G && { ...; exit; }`, `G || exit`).
// Not recognised: a guard held in a variable (`v=${BASH_VERSINFO[0]}; ((v >= 4))`), `case`
// on `BASH_VERSION`, and a `[[ ]]` string comparison such as `>=`, which is lexical there.

use shelllint_core::ast::{Ast, Id, Token};
use shelllint_core::astlib::get_command_name;

/// A release as `(major, minor)`. Tuples compare as releases do.
pub type Version = (u32, u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Major,
    Minor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rel {
    /// `field >= n`.
    Ge,
    /// `field < n`.
    Lt,
    /// `field == n`. It proves `>= n` when true and nothing when false.
    Eq,
}

#[derive(Debug)]
enum Guard {
    Atom(Field, Rel, u32),
    Not(Box<Guard>),
    And(Box<Guard>, Box<Guard>),
    Or(Box<Guard>, Box<Guard>),
    Unknown,
}

/// Lower bounds on each field, for the facts a condition proves. Merging two sets keeps the
/// larger bound of each field, since both facts hold when both conditions hold.
#[derive(Clone, Copy, Default)]
struct Bounds {
    major: Option<u32>,
    minor: Option<u32>,
}

impl Bounds {
    fn of(field: Field, n: u32) -> Bounds {
        match field {
            Field::Major => Bounds { major: Some(n), minor: None },
            Field::Minor => Bounds { major: None, minor: Some(n) },
        }
    }

    fn merge(self, other: Bounds) -> Bounds {
        Bounds { major: max_opt(self.major, other.major), minor: max_opt(self.minor, other.minor) }
    }

    /// A release is only proved by a major bound. A minor bound alone says nothing about it.
    fn version(self) -> Option<Version> {
        self.major.map(|major| (major, self.minor.unwrap_or(0)))
    }
}

fn max_opt(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) | (None, x) => x,
    }
}

impl Guard {
    /// The facts that hold when this guard is true.
    fn when_true(&self) -> Bounds {
        match self {
            Guard::Atom(field, Rel::Ge | Rel::Eq, n) => Bounds::of(*field, *n),
            Guard::And(a, b) => a.when_true().merge(b.when_true()),
            Guard::Not(x) => x.when_false(),
            Guard::Atom(..) | Guard::Or(..) | Guard::Unknown => Bounds::default(),
        }
    }

    /// The facts that hold when this guard is false.
    fn when_false(&self) -> Bounds {
        match self {
            Guard::Atom(field, Rel::Lt, n) => Bounds::of(*field, *n),
            Guard::Or(a, b) => a.when_false().merge(b.when_false()),
            Guard::Not(x) => x.when_true(),
            Guard::Atom(..) | Guard::And(..) | Guard::Unknown => Bounds::default(),
        }
    }

    /// The least release that a true guard proves. For `a || b` both sides must prove one, and the
    /// smaller of the two is what holds whichever side is true.
    fn min_when_true(&self) -> Option<Version> {
        match self {
            Guard::Or(a, b) => a.min_when_true().zip(b.min_when_true()).map(|(x, y)| x.min(y)),
            _ => self.when_true().version(),
        }
    }

    /// The least release that a false guard proves. Dually, `a && b` false needs one side false.
    fn min_when_false(&self) -> Option<Version> {
        match self {
            Guard::And(a, b) => a.min_when_false().zip(b.min_when_false()).map(|(x, y)| x.min(y)),
            _ => self.when_false().version(),
        }
    }
}

/// The least release that a version guard around `t` proves, or `None` when no guard applies.
/// Walks the ancestors of `t`, so nested guards and function bodies inherit their guards.
pub fn floor(ast: &Ast, t: Id) -> Option<Version> {
    let mut best = None;
    let mut child = t;
    while let Some(parent) = ast.parent(child) {
        best = best.max(edge_floor(ast, parent, child));
        child = parent;
    }
    best
}

/// Whether a version guard around `t` proves `release` or later.
pub fn covers(ast: &Ast, t: Id, release: (u8, u8)) -> bool {
    floor(ast, t).is_some_and(|(major, minor)| (major, minor) >= (u32::from(release.0), u32::from(release.1)))
}

/// What the edge from `parent` down to its child `c` proves: a branch or `&&` / `||` guard that
/// `c` sits under, or an early exit in an earlier sibling.
fn edge_floor(ast: &Ast, parent: Id, c: Id) -> Option<Version> {
    branch_floor(ast, parent, c).max(sibling_floor(ast, parent, c))
}

fn branch_floor(ast: &Ast, parent: Id, c: Id) -> Option<Version> {
    match ast.get(parent) {
        Token::T_IfExpression(conds, elses) => {
            if let Some(i) = conds.iter().position(|(_, body)| body.contains(&c)) {
                // Body `i` runs when condition `i` is true and every earlier condition was false.
                let mut best = guard_of_list(ast, &conds[i].0).min_when_true();
                for (cond, _) in &conds[..i] {
                    best = best.max(guard_of_list(ast, cond).min_when_false());
                }
                best
            } else if elses.contains(&c) {
                conds.iter().fold(None, |best, (cond, _)| best.max(guard_of_list(ast, cond).min_when_false()))
            } else {
                None
            }
        }
        // `c` is the right side of `a && c`, which runs only when `a` is true.
        Token::T_AndIf(a, b) if *b == c => parse(ast, *a).min_when_true(),
        // `c` is the right side of `a || c`, which runs only when `a` is false.
        Token::T_OrIf(a, b) if *b == c => parse(ast, *a).min_when_false(),
        _ => None,
    }
}

/// The guard of a condition list, which is its last statement.
fn guard_of_list(ast: &Ast, list: &[Id]) -> Guard {
    list.last().map_or(Guard::Unknown, |&s| parse(ast, s))
}

/// Early exits in the statements before `c` in the same list. Once one of them is passed without
/// exiting, its guard is known to be false (or true, for `||`).
fn sibling_floor(ast: &Ast, parent: Id, c: Id) -> Option<Version> {
    let list = statement_list(ast, parent, c)?;
    let at = list.iter().position(|&s| s == c)?;
    list[..at].iter().map(|&s| early_exit_floor(ast, s)).fold(None, |best, f| best.max(f))
}

/// The statement list that holds `c` directly, if `parent` is a container of statements.
fn statement_list(ast: &Ast, parent: Id, c: Id) -> Option<&[Id]> {
    let list: &[Id] = match ast.get(parent) {
        Token::T_Script(_, l) | Token::T_BraceGroup(l) | Token::T_Subshell(l) => l.as_slice(),
        Token::T_WhileExpression(_, l) | Token::T_UntilExpression(_, l) => l.as_slice(),
        Token::T_ForIn(_, _, l) | Token::T_SelectIn(_, _, l) | Token::T_ForArithmetic(_, _, _, l) => l.as_slice(),
        Token::T_IfExpression(conds, elses) => {
            return conds
                .iter()
                .map(|(_, body)| body.as_slice())
                .chain(std::iter::once(elses.as_slice()))
                .find(|l| l.contains(&c));
        }
        Token::T_CaseExpression(_, clauses) => {
            return clauses.iter().map(|(_, _, body)| body.as_slice()).find(|l| l.contains(&c));
        }
        _ => return None,
    };
    list.contains(&c).then_some(list)
}

/// The release that holds after statement `s` when it is an early exit guard.
fn early_exit_floor(ast: &Ast, s: Id) -> Option<Version> {
    let x = unwrap_stmt(ast, s);
    match ast.get(x) {
        // `if NOT_G; then ...; exit; fi`: after it, G is true, so NOT_G is false.
        Token::T_IfExpression(conds, elses) => match conds.as_slice() {
            [(cond, body)] if elses.is_empty() && ends_in_exit(ast, body) => {
                guard_of_list(ast, cond).min_when_false()
            }
            _ => None,
        },
        // `NOT_G && { ...; exit; }`: reaching the next statement, the `&&` left side is false.
        Token::T_AndIf(a, b) if exits(ast, *b) => parse(ast, *a).min_when_false(),
        // `G || exit`: reaching the next statement, G is true.
        Token::T_OrIf(a, b) if exits(ast, *b) => parse(ast, *a).min_when_true(),
        _ => None,
    }
}

fn ends_in_exit(ast: &Ast, list: &[Id]) -> bool {
    list.last().is_some_and(|&s| exits(ast, s))
}

/// Whether statement `s` always ends the script or function with `exit` or `return`.
fn exits(ast: &Ast, s: Id) -> bool {
    let x = unwrap_stmt(ast, s);
    match ast.get(x) {
        Token::T_SimpleCommand(..) => matches!(get_command_name(ast, x).as_deref(), Some("exit" | "return")),
        Token::T_BraceGroup(list) => ends_in_exit(ast, list),
        _ => false,
    }
}

/// The command under a statement's pipeline and redirection wrappers, when they add nothing.
fn unwrap_stmt(ast: &Ast, mut id: Id) -> Id {
    loop {
        id = match ast.get(id) {
            Token::T_Pipeline(pipes, cmds) if pipes.is_empty() && cmds.len() == 1 => cmds[0],
            Token::T_Redirecting(redirs, cmd) if redirs.is_empty() => *cmd,
            _ => return id,
        };
    }
}

/// The kind of comparison, which decides the operators and operand forms it accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// `(( ))`: operands are arithmetic, so a bare name is a variable.
    Arith,
    /// `[[ ]]`: a bare name is arithmetic, and `==` is allowed.
    DoubleBracket,
    /// `[ ]` and `test`: operands are words, so a version needs a `$` expansion.
    Test,
}

fn parse(ast: &Ast, id: Id) -> Guard {
    let id = unwrap_stmt(ast, id);
    match ast.get(id) {
        Token::T_AndIf(a, b) => Guard::And(Box::new(parse(ast, *a)), Box::new(parse(ast, *b))),
        Token::T_OrIf(a, b) => Guard::Or(Box::new(parse(ast, *a)), Box::new(parse(ast, *b))),
        Token::T_Banged(x) => Guard::Not(Box::new(parse(ast, *x))),
        Token::T_Arithmetic(x) => arith(ast, *x),
        Token::T_Condition(_, _, x) => cond(ast, *x),
        Token::T_SimpleCommand(_, words) if get_command_name(ast, id).as_deref() == Some("test") => {
            test(ast, words)
        }
        _ => Guard::Unknown,
    }
}

fn arith(ast: &Ast, id: Id) -> Guard {
    match ast.get(id) {
        Token::TA_Sequence(l) if l.len() == 1 => arith(ast, l[0]),
        Token::TA_Parenthesis(x) => arith(ast, *x),
        Token::TA_Unary(op, x) if op == "!" => Guard::Not(Box::new(arith(ast, *x))),
        Token::TA_Binary(op, a, b) if op == "&&" => Guard::And(Box::new(arith(ast, *a)), Box::new(arith(ast, *b))),
        Token::TA_Binary(op, a, b) if op == "||" => Guard::Or(Box::new(arith(ast, *a)), Box::new(arith(ast, *b))),
        Token::TA_Binary(op, a, b) => compare(ast, Ctx::Arith, op, *a, *b),
        _ => Guard::Unknown,
    }
}

fn cond(ast: &Ast, id: Id) -> Guard {
    match ast.get(id) {
        Token::TC_And(_, op, a, b) if op == "&&" || op == "-a" => {
            Guard::And(Box::new(cond(ast, *a)), Box::new(cond(ast, *b)))
        }
        Token::TC_Or(_, op, a, b) if op == "||" || op == "-o" => {
            Guard::Or(Box::new(cond(ast, *a)), Box::new(cond(ast, *b)))
        }
        Token::TC_Unary(_, op, x) if op == "!" => Guard::Not(Box::new(cond(ast, *x))),
        Token::TC_Group(_, x) => cond(ast, *x),
        Token::TC_Binary(kind, op, a, b) => {
            let ctx = if matches!(kind, shelllint_core::ast::ConditionType::DoubleBracket) {
                Ctx::DoubleBracket
            } else {
                Ctx::Test
            };
            compare(ast, ctx, op, *a, *b)
        }
        _ => Guard::Unknown,
    }
}

/// `[ ]` and `test` with an optional `!`: `test ! X -ge 4` is `!` around the comparison.
fn test(ast: &Ast, words: &[Id]) -> Guard {
    let args = &words[1..];
    let (negated, args) = match args.first().and_then(|&w| plain_text(ast, w)) {
        Some((s, false)) if s == "!" => (true, &args[1..]),
        _ => (false, args),
    };
    let [lhs, op, rhs] = args else { return Guard::Unknown };
    let Some((op, _)) = plain_text(ast, *op) else { return Guard::Unknown };
    let guard = compare(ast, Ctx::Test, &op, *lhs, *rhs);
    if negated { Guard::Not(Box::new(guard)) } else { guard }
}

/// A comparison of a version field with a literal, in either order.
fn compare(ast: &Ast, ctx: Ctx, op: &str, a: Id, b: Id) -> Guard {
    let allowed = match ctx {
        Ctx::Arith => matches!(op, ">=" | ">" | "==" | "<" | "<="),
        Ctx::DoubleBracket => matches!(op, "-ge" | "-gt" | "-eq" | "-lt" | "-le" | "=="),
        Ctx::Test => matches!(op, "-ge" | "-gt" | "-eq" | "-lt" | "-le"),
    };
    if !allowed {
        return Guard::Unknown;
    }
    let bare = ctx != Ctx::Test;
    if let (Some(field), Some(n)) = (field(ast, a, bare), literal(ast, b)) {
        return relation(field, op, n, false);
    }
    if let (Some(n), Some(field)) = (literal(ast, a), field(ast, b, bare)) {
        return relation(field, op, n, true);
    }
    Guard::Unknown
}

/// `field op n`. With the literal on the left, `n op field` is `field flip(op) n`.
fn relation(field: Field, op: &str, n: u32, literal_first: bool) -> Guard {
    let op = if literal_first { flip(op) } else { op };
    match op {
        ">=" | "-ge" => Guard::Atom(field, Rel::Ge, n),
        // `x > n` is `x >= n + 1` for integers. The literal has at most nine digits, so this fits.
        ">" | "-gt" => Guard::Atom(field, Rel::Ge, n + 1),
        "==" | "-eq" => Guard::Atom(field, Rel::Eq, n),
        "<" | "-lt" => Guard::Atom(field, Rel::Lt, n),
        "<=" | "-le" => Guard::Atom(field, Rel::Lt, n + 1),
        _ => Guard::Unknown,
    }
}

fn flip(op: &str) -> &str {
    match op {
        ">=" => "<=",
        "<=" => ">=",
        ">" => "<",
        "<" => ">",
        "-ge" => "-le",
        "-le" => "-ge",
        "-gt" => "-lt",
        "-lt" => "-gt",
        other => other,
    }
}

/// The version field that operand `id` names, when it is one of the recognised spellings.
fn field(ast: &Ast, id: Id, bare_ok: bool) -> Option<Field> {
    let (text, dollar) = plain_text(ast, id)?;
    if !dollar && !bare_ok {
        return None;
    }
    let field = match text.as_str() {
        "BASH_VERSINFO" | "BASH_VERSINFO[0]" | "BASH_VERSION%%.*" | "BASH_VERSION:0:1" => Field::Major,
        "BASH_VERSINFO[1]" => Field::Minor,
        _ => return None,
    };
    // A bare `BASH_VERSION...` is a string, not arithmetic over the version.
    if !dollar && text.starts_with("BASH_VERSION") {
        return None;
    }
    Some(field)
}

/// A literal non-negative integer of at most nine digits, unexpanded.
fn literal(ast: &Ast, id: Id) -> Option<u32> {
    let (text, dollar) = plain_text(ast, id)?;
    if dollar || text.is_empty() || text.len() > 9 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The text of a word or arithmetic operand, and whether a `$` expansion supplies it. A word
/// counts only when it is one expansion or only literal text, so `$V[0]` never reads as `${V[0]}`.
fn plain_text(ast: &Ast, id: Id) -> Option<(String, bool)> {
    match ast.get(id) {
        Token::T_Literal(s) | Token::T_Glob(s) => Some((s.clone(), false)),
        Token::T_DollarBraced(_, word) => plain_text(ast, *word).map(|(s, _)| (s, true)),
        Token::T_NormalWord(parts) | Token::T_DoubleQuoted(parts) | Token::TA_Sequence(parts) | Token::TA_Expansion(parts) => {
            match parts.as_slice() {
                [only] => plain_text(ast, *only),
                _ => {
                    let mut text = String::new();
                    for &part in parts {
                        let (s, dollar) = plain_text(ast, part)?;
                        if dollar {
                            return None;
                        }
                        text.push_str(&s);
                    }
                    Some((text, false))
                }
            }
        }
        Token::TA_Variable(name, indices) => match indices.as_slice() {
            [] => Some((name.clone(), false)),
            [index] => {
                let (text, dollar) = plain_text(ast, *index)?;
                (!dollar).then(|| (format!("{name}[{text}]"), false))
            }
            _ => None,
        },
        _ => None,
    }
}
