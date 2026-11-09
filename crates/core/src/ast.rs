// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/AST.hs (e11f964)
//
// DIVERGENCE: Haskell's `Token = OuterToken Id (InnerToken Token)` is a tree of
// boxed values. Here every token lives in an arena (`Ast`) and refers to its
// children by `Id`; the `Inner_` prefix is dropped from the constructor names
// and `OuterToken` itself is implicit (the arena index is the token's Id).
#![allow(non_camel_case_types)]

use crate::prelude::please_report;

/// `newtype Id = Id Int`
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Id(pub u32);

/// Parsec's `SourcePos` without the file name. 1-based line and column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Pos {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Quoted {
    Quoted,
    Unquoted,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dashed {
    Dashed,
    Undashed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Piped {
    Piped,
    Unpiped,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssignmentMode {
    Assign,
    Append,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FunctionKeyword(pub bool);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FunctionParentheses(pub bool);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CaseType {
    CaseBreak,
    CaseFallThrough,
    CaseContinue,
}

// DIVERGENCE: `newtype Root = Root Token` is not ported; later tasks pass the
// root `Id` alongside the `Ast`.

/// `InnerToken t`, with `t = Id`. Field order is exactly the Haskell field
/// order, which is also the order Haskell's derived `Traversable` visits.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Token {
    TA_Binary(String, Id, Id),
    TA_Assignment(String, Id, Id),
    TA_Variable(String, Vec<Id>),
    TA_Expansion(Vec<Id>),
    TA_Sequence(Vec<Id>),
    TA_Parenthesis(Id),
    TA_Trinary(Id, Id, Id),
    TA_Unary(String, Id),
    TC_And(ConditionType, String, Id, Id),
    TC_Binary(ConditionType, String, Id, Id),
    TC_Group(ConditionType, Id),
    TC_Nullary(ConditionType, Id),
    TC_Or(ConditionType, String, Id, Id),
    TC_Unary(ConditionType, String, Id),
    TC_Empty(ConditionType),
    T_AND_IF,
    T_AndIf(Id, Id),
    T_Arithmetic(Id),
    T_Array(Vec<Id>),
    T_IndexedElement(Vec<Id>, Id),
    /// Store the index as string, and parse as arithmetic or string later
    T_UnparsedIndex(Pos, String),
    T_Assignment(AssignmentMode, String, Vec<Id>, Id),
    T_Backgrounded(Id),
    T_Backticked(Vec<Id>),
    T_Bang,
    T_Banged(Id),
    T_BraceExpansion(Vec<Id>),
    T_BraceGroup(Vec<Id>),
    T_CLOBBER,
    T_Case,
    T_CaseExpression(Id, Vec<(CaseType, Vec<Id>, Vec<Id>)>),
    T_Condition(ConditionType, ConditionType, Id),
    T_DGREAT,
    T_DLESS,
    T_DLESSDASH,
    T_DSEMI,
    T_Do,
    T_DollarArithmetic(Id),
    T_DollarBraced(bool, Id),
    T_DollarBracket(Id),
    T_DollarDoubleQuoted(Vec<Id>),
    T_DollarExpansion(Vec<Id>),
    T_DollarSingleQuoted(String),
    T_DollarBraceCommandExpansion(Piped, Vec<Id>),
    T_Done,
    T_DoubleQuoted(Vec<Id>),
    T_EOF,
    T_Elif,
    T_Else,
    T_Esac,
    T_Extglob(String, Vec<Id>),
    T_FdRedirect(String, Id),
    T_Fi,
    T_For,
    T_ForArithmetic(Id, Id, Id, Vec<Id>),
    T_ForIn(String, Vec<Id>, Vec<Id>),
    T_Function(FunctionKeyword, FunctionParentheses, String, Id),
    T_GREATAND,
    T_Glob(String),
    T_Greater,
    T_HereDoc(Dashed, Quoted, String, Vec<Id>),
    T_HereString(Id),
    T_If,
    T_IfExpression(Vec<(Vec<Id>, Vec<Id>)>, Vec<Id>),
    T_In,
    T_IoFile(Id, Id),
    T_IoDuplicate(Id, String),
    T_LESSAND,
    T_LESSGREAT,
    T_Lbrace,
    T_Less,
    T_Literal(String),
    T_Lparen,
    T_NEWLINE,
    T_NormalWord(Vec<Id>),
    T_OR_IF,
    T_OrIf(Id, Id),
    /// e.g. '%' in ${foo%bar}  or '/' in ${foo/bar/baz}
    T_ParamSubSpecialChar(String),
    /// [Pipe separators] [Commands]
    T_Pipeline(Vec<Id>, Vec<Id>),
    T_ProcSub(String, Vec<Id>),
    T_Rbrace,
    T_Redirecting(Vec<Id>, Id),
    T_Rparen,
    /// Shebang T_Literal, followed by script.
    T_Script(Id, Vec<Id>),
    T_Select,
    T_SelectIn(String, Vec<Id>, Vec<Id>),
    T_Semi,
    T_SimpleCommand(Vec<Id>, Vec<Id>),
    T_SingleQuoted(String),
    T_Subshell(Vec<Id>),
    T_Then,
    T_Until,
    T_UntilExpression(Vec<Id>, Vec<Id>),
    T_While,
    T_WhileExpression(Vec<Id>, Vec<Id>),
    T_Annotation(Vec<Annotation>, Id),
    T_Pipe(String),
    /// The name is a `Maybe Token`, not a `t`: it is NOT visited by
    /// `Traversable` (so not a child), but it IS compared by `Eq`.
    T_CoProc(Option<Id>, Id),
    T_CoProcBody(Id),
    T_Include(Id),
    T_SourceCommand(Id, Id),
    T_BatsTest(String, Id),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Annotation {
    /// [from, to)
    DisableComment(i64, i64),
    EnableComment(String),
    SourceOverride(String),
    ShellOverride(String),
    SourcePath(String),
    ExternalSources(bool),
    ExtendedAnalysis(bool),
    // DIVERGENCE: the variants below are not in upstream.
    /// `severity=` (rc file or file-level directive)
    MinSeverity(crate::interface::Severity),
    /// One name of `external-vars=` (a trailing `*` is a prefix glob)
    ExternalVar(String),
    /// The shell named by `#! nix-shell -i <shell>` after a nix-shell shebang.
    /// Weaker than `ShellOverride`.
    ShellFromNix(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ConditionType {
    DoubleBracket,
    SingleBracket,
}

/// Visit every `t` field of a token in derived-`Traversable` order: fields
/// left to right, lists in order, tuples left to right. Works on both `&Token`
/// and `&mut Token` thanks to match ergonomics. No `_` arm on purpose.
macro_rules! for_each_child {
    ($tok:expr, $f:ident) => {
        match $tok {
            Token::TA_Binary(_, a, b) => {
                $f(a);
                $f(b);
            }
            Token::TA_Assignment(_, a, b) => {
                $f(a);
                $f(b);
            }
            Token::TA_Variable(_, l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::TA_Expansion(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::TA_Sequence(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::TA_Parenthesis(a) => {
                $f(a);
            }
            Token::TA_Trinary(a, b, c) => {
                $f(a);
                $f(b);
                $f(c);
            }
            Token::TA_Unary(_, a) => {
                $f(a);
            }
            Token::TC_And(_, _, a, b) => {
                $f(a);
                $f(b);
            }
            Token::TC_Binary(_, _, a, b) => {
                $f(a);
                $f(b);
            }
            Token::TC_Group(_, a) => {
                $f(a);
            }
            Token::TC_Nullary(_, a) => {
                $f(a);
            }
            Token::TC_Or(_, _, a, b) => {
                $f(a);
                $f(b);
            }
            Token::TC_Unary(_, _, a) => {
                $f(a);
            }
            Token::TC_Empty(_) => {}
            Token::T_AND_IF => {}
            Token::T_AndIf(a, b) => {
                $f(a);
                $f(b);
            }
            Token::T_Arithmetic(a) => {
                $f(a);
            }
            Token::T_Array(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_IndexedElement(l, a) => {
                for x in l {
                    $f(x);
                }
                $f(a);
            }
            Token::T_UnparsedIndex(_, _) => {}
            Token::T_Assignment(_, _, l, a) => {
                for x in l {
                    $f(x);
                }
                $f(a);
            }
            Token::T_Backgrounded(a) => {
                $f(a);
            }
            Token::T_Backticked(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_Bang => {}
            Token::T_Banged(a) => {
                $f(a);
            }
            Token::T_BraceExpansion(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_BraceGroup(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_CLOBBER => {}
            Token::T_Case => {}
            Token::T_CaseExpression(a, cases) => {
                $f(a);
                for (_, pats, body) in cases {
                    for x in pats {
                        $f(x);
                    }
                    for x in body {
                        $f(x);
                    }
                }
            }
            Token::T_Condition(_, _, a) => {
                $f(a);
            }
            Token::T_DGREAT => {}
            Token::T_DLESS => {}
            Token::T_DLESSDASH => {}
            Token::T_DSEMI => {}
            Token::T_Do => {}
            Token::T_DollarArithmetic(a) => {
                $f(a);
            }
            Token::T_DollarBraced(_, a) => {
                $f(a);
            }
            Token::T_DollarBracket(a) => {
                $f(a);
            }
            Token::T_DollarDoubleQuoted(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_DollarExpansion(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_DollarSingleQuoted(_) => {}
            Token::T_DollarBraceCommandExpansion(_, l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_Done => {}
            Token::T_DoubleQuoted(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_EOF => {}
            Token::T_Elif => {}
            Token::T_Else => {}
            Token::T_Esac => {}
            Token::T_Extglob(_, l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_FdRedirect(_, a) => {
                $f(a);
            }
            Token::T_Fi => {}
            Token::T_For => {}
            Token::T_ForArithmetic(a, b, c, l) => {
                $f(a);
                $f(b);
                $f(c);
                for x in l {
                    $f(x);
                }
            }
            Token::T_ForIn(_, l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_Function(_, _, _, a) => {
                $f(a);
            }
            Token::T_GREATAND => {}
            Token::T_Glob(_) => {}
            Token::T_Greater => {}
            Token::T_HereDoc(_, _, _, l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_HereString(a) => {
                $f(a);
            }
            Token::T_If => {}
            Token::T_IfExpression(conds, elses) => {
                for (c, b) in conds {
                    for x in c {
                        $f(x);
                    }
                    for x in b {
                        $f(x);
                    }
                }
                for x in elses {
                    $f(x);
                }
            }
            Token::T_In => {}
            Token::T_IoFile(a, b) => {
                $f(a);
                $f(b);
            }
            Token::T_IoDuplicate(a, _) => {
                $f(a);
            }
            Token::T_LESSAND => {}
            Token::T_LESSGREAT => {}
            Token::T_Lbrace => {}
            Token::T_Less => {}
            Token::T_Literal(_) => {}
            Token::T_Lparen => {}
            Token::T_NEWLINE => {}
            Token::T_NormalWord(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_OR_IF => {}
            Token::T_OrIf(a, b) => {
                $f(a);
                $f(b);
            }
            Token::T_ParamSubSpecialChar(_) => {}
            Token::T_Pipeline(l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_ProcSub(_, l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_Rbrace => {}
            Token::T_Redirecting(l, a) => {
                for x in l {
                    $f(x);
                }
                $f(a);
            }
            Token::T_Rparen => {}
            Token::T_Script(a, l) => {
                $f(a);
                for x in l {
                    $f(x);
                }
            }
            Token::T_Select => {}
            Token::T_SelectIn(_, l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_Semi => {}
            Token::T_SimpleCommand(l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_SingleQuoted(_) => {}
            Token::T_Subshell(l) => {
                for x in l {
                    $f(x);
                }
            }
            Token::T_Then => {}
            Token::T_Until => {}
            Token::T_UntilExpression(l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_While => {}
            Token::T_WhileExpression(l1, l2) => {
                for x in l1 {
                    $f(x);
                }
                for x in l2 {
                    $f(x);
                }
            }
            Token::T_Annotation(_, a) => {
                $f(a);
            }
            Token::T_Pipe(_) => {}
            // The `Maybe Token` name is not a `t`, so it is not traversed.
            Token::T_CoProc(_, a) => {
                $f(a);
            }
            Token::T_CoProcBody(a) => {
                $f(a);
            }
            Token::T_Include(a) => {
                $f(a);
            }
            Token::T_SourceCommand(a, b) => {
                $f(a);
                $f(b);
            }
            Token::T_BatsTest(_, a) => {
                $f(a);
            }
        }
    };
}

impl Token {
    /// The `t` fields of this token, in derived-`Traversable` order.
    pub fn children(&self) -> Vec<Id> {
        let mut out = Vec::new();
        let mut f = |x: &Id| out.push(*x);
        for_each_child!(self, f);
        out
    }

    /// Haskell's derived `Functor`: apply `g` to every `t` field.
    fn map_children(&mut self, g: &mut dyn FnMut(Id) -> Id) {
        let mut f = |x: &mut Id| *x = g(*x);
        for_each_child!(self, f);
    }
}

/// Arena holding every token of a parse. `Id(n)` indexes the n-th slot.
#[derive(Clone, Debug, Default)]
pub struct Ast {
    tokens: Vec<Option<Token>>,
    spans: Vec<(Pos, Pos)>,
    parents: Vec<Option<Id>>,
}

impl Ast {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve the next id with the given span; the token is filled by `set`.
    pub fn alloc(&mut self, start: Pos, end: Pos) -> Id {
        let id = Id(self.len());
        self.tokens.push(None);
        self.spans.push((start, end));
        self.parents.push(None);
        id
    }

    pub fn set(&mut self, id: Id, t: Token) {
        self.tokens[id.0 as usize] = Some(t);
    }

    pub fn get(&self, id: Id) -> &Token {
        match &self.tokens[id.0 as usize] {
            Some(t) => t,
            None => panic!("{}", please_report(&format!("token {:?} allocated but never set", id))),
        }
    }

    pub fn span(&self, id: Id) -> (Pos, Pos) {
        self.spans[id.0 as usize]
    }

    pub fn len(&self) -> u32 {
        u32::try_from(self.tokens.len()).expect("more than u32::MAX tokens")
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Drop every id >= `len` (parser backtracking).
    pub fn truncate(&mut self, len: u32) {
        let len = len as usize;
        self.tokens.truncate(len);
        self.spans.truncate(len);
        self.parents.truncate(len);
    }

    /// Children in the order Haskell's derived `Traversable` visits them.
    pub fn children(&self, id: Id) -> impl Iterator<Item = Id> + use<> {
        self.get(id).children().into_iter()
    }

    /// Record the parent of every token reachable from `root`.
    pub fn compute_parents(&mut self, root: Id) {
        self.parents.iter_mut().for_each(|p| *p = None);
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            for child in self.get(id).children() {
                self.parents[child.0 as usize] = Some(id);
                stack.push(child);
            }
        }
    }

    pub fn parent(&self, id: Id) -> Option<Id> {
        self.parents[id.0 as usize]
    }

    /// `analyze f g return`: call `pre` on a token, recurse into its children
    /// in `Traversable` order, then call `post` on it.
    // DIVERGENCE: iterative rather than recursive, so deep trees cannot
    // overflow the Rust stack; the visiting order is identical.
    pub fn analyze(&self, root: Id, pre: &mut dyn FnMut(Id), post: &mut dyn FnMut(Id)) {
        pre(root);
        let mut stack: Vec<(Id, std::vec::IntoIter<Id>)> = vec![(root, self.get(root).children().into_iter())];
        while let Some((id, iter)) = stack.last_mut() {
            match iter.next() {
                Some(child) => {
                    pre(child);
                    let it = self.get(child).children().into_iter();
                    stack.push((child, it));
                }
                None => {
                    let id = *id;
                    stack.pop();
                    post(id);
                }
            }
        }
    }

    /// `instance Eq Token where OuterToken _ a == OuterToken _ b = a == b`,
    /// applied recursively: compares tokens ignoring their ids.
    pub fn structurally_eq(&self, a: Id, b: Id) -> bool {
        let (ta, tb) = (self.get(a), self.get(b));
        if let (Token::T_CoProc(n1, b1), Token::T_CoProc(n2, b2)) = (ta, tb) {
            // `Maybe Token` is compared with `Eq Token` too.
            let names_eq = match (n1, n2) {
                (None, None) => true,
                (Some(x), Some(y)) => self.structurally_eq(*x, *y),
                _ => false,
            };
            return names_eq && self.structurally_eq(*b1, *b2);
        }
        // Compare everything except the `t` fields, then the `t` fields
        // pairwise. Equal shapes imply equal child counts.
        let mut na = ta.clone();
        let mut nb = tb.clone();
        na.map_children(&mut |_| Id(0));
        nb.map_children(&mut |_| Id(0));
        na == nb && ta.children().into_iter().zip(tb.children()).all(|(x, y)| self.structurally_eq(x, y))
    }
}

// DIVERGENCE: `getId` is not needed (the arena index is the id); `blank`,
// `doAnalysis`, `doStackAnalysis` are covered by `Ast::analyze`; `doTransform`
// is replaced by `Ast::set` on the arena.
