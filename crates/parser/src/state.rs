// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 156-463:
// parse state, ids and spans, here-doc bookkeeping, parse notes and problems,
// the context stack, and the convenience combinators.
//
// DIVERGENCE: Haskell lists that are only ever prepended to and rolled back
// in bulk (`parseNotes`, `parseProblems`) are stored oldest-first in a Vec, so
// backtracking can truncate them; the Haskell order is the reverse.
// `contextStack` and `pendingHereDocs` are shared cons lists (`PList`) in the
// Haskell order (innermost / newest first), so snapshots of them are O(1) as
// in Haskell.
//
// DIVERGENCE: `positionMap` lives in the `Ast` arena (`Ast::span`), with the
// file of each end kept alongside in `UserState`; `lastId` is derived from the
// arena length. Ids are allocated at exactly the same points as the Haskell.
//
// DIVERGENCE: state accessors that are infallible parser actions in Haskell
// (`getNextIdBetween`, `parseProblem`, ...) return plain values here; they
// can never fail, and their only Parsec effect (an empty success with an
// unknown error) leaves the carried error unchanged.

use std::collections::BTreeMap;
use std::rc::Rc;

use shellcheck_core::ast::{Annotation, Ast, Dashed, Id, Quoted, Token};
use shellcheck_core::interface::{Code, Severity, Shell, SystemInterface};
use shellcheck_core::prelude::please_report;

use crate::engine::{FileId, P, PR, SourcePos};

//--------- Message/position annotation on top of user state

/// `data ParseNote = ParseNote SourcePos SourcePos Severity Code String`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseNote {
    pub start: SourcePos,
    pub end: SourcePos,
    pub severity: Severity,
    pub code: Code,
    pub message: String,
}

impl ParseNote {
    pub fn new(start: SourcePos, end: SourcePos, severity: Severity, code: Code, message: &str) -> Self {
        ParseNote { start, end, severity, code, message: message.to_string() }
    }
}

/// An immutable Haskell-style list whose clones share structure, so copying
/// one (a snapshot, `Ms.get`) is O(1). Iteration is head (newest) first.
pub struct PList<T>(Option<Rc<PNode<T>>>);

struct PNode<T> {
    head: T,
    tail: PList<T>,
}

impl<T> PList<T> {
    pub const fn new() -> Self {
        PList(None)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// O(n); for tests and diagnostics.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// `x : list`
    pub fn push(&mut self, head: T) {
        let tail = std::mem::take(self);
        self.0 = Some(Rc::new(PNode { head, tail }));
    }

    /// Drop the head; `tail` (no-op on an empty list).
    pub fn drop_head(&mut self) {
        if let Some(rc) = self.0.take() {
            match Rc::try_unwrap(rc) {
                Ok(mut node) => *self = std::mem::take(&mut node.tail),
                Err(rc) => *self = rc.tail.clone(),
            }
        }
    }

    pub fn iter(&self) -> PListIter<'_, T> {
        PListIter(self.0.as_deref())
    }

    /// Are these the same list object (an O(1) equality shortcut)?
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl<T> Default for PList<T> {
    fn default() -> Self {
        PList(None)
    }
}

impl<T> Clone for PList<T> {
    fn clone(&self) -> Self {
        PList(self.0.clone())
    }
}

impl<T> Drop for PList<T> {
    // Iterative, so a long unshared list does not recurse once per node.
    fn drop(&mut self) {
        let mut cur = self.0.take();
        while let Some(rc) = cur {
            match Rc::try_unwrap(rc) {
                Ok(mut node) => cur = node.tail.0.take(),
                Err(_) => break,
            }
        }
    }
}

impl<T: PartialEq> PartialEq for PList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for PList<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for PList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T> FromIterator<T> for PList<T> {
    /// The first item becomes the head.
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let items: Vec<T> = iter.into_iter().collect();
        let mut list = PList::new();
        for x in items.into_iter().rev() {
            list.push(x);
        }
        list
    }
}

pub struct PListIter<'a, T>(Option<&'a PNode<T>>);

impl<'a, T> Iterator for PListIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        let node = self.0?;
        self.0 = node.tail.0.as_deref();
        Some(&node.head)
    }
}

/// `contextStack`: innermost first, shared on copy.
pub type ContextStack = PList<Context>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    // DIVERGENCE: the name is always a literal, so it is not allocated.
    ContextName(SourcePos, &'static str),
    ContextAnnotation(Vec<Annotation>),
    ContextSource(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HereDocContext {
    /// on linefeed, read this T_HereDoc
    HereDocPending(Id, Dashed, Quoted, String, ContextStack),
}

/// `data UserState`: everything that rolls back on backtracking.
#[derive(Clone, Debug, Default)]
pub struct UserState {
    /// The token arena; also `positionMap` (spans) and `lastId` (length).
    pub ast: Ast,
    /// File of each span's (start, end); parallel to `ast`.
    pub(crate) span_files: Vec<(FileId, FileId)>,
    /// Oldest first.
    pub parse_notes: Vec<ParseNote>,
    /// `Map.insert` log, oldest first; later entries win.
    pub here_doc_map: Vec<(Id, Vec<Id>)>,
    /// Newest first, as in Haskell; shared on copy.
    pub pending_here_docs: PList<HereDocContext>,
}

/// What a checkpoint needs to restore a `UserState`: lengths to truncate back
/// to, plus the (shared, O(1)) pending here-doc list.
#[derive(Clone, Debug)]
pub struct UserSnapshot {
    ast_len: u32,
    parse_notes_len: usize,
    here_doc_map_len: usize,
    pending_here_docs: PList<HereDocContext>,
}

impl UserSnapshot {
    pub(crate) fn ast_len(&self) -> u32 {
        self.ast_len
    }
}

impl UserState {
    pub fn snapshot(&self) -> UserSnapshot {
        UserSnapshot {
            ast_len: self.ast.len(),
            parse_notes_len: self.parse_notes.len(),
            here_doc_map_len: self.here_doc_map.len(),
            pending_here_docs: self.pending_here_docs.clone(),
        }
    }

    pub fn restore(&mut self, s: &UserSnapshot) {
        self.ast.truncate(s.ast_len);
        self.span_files.truncate(s.ast_len as usize);
        self.parse_notes.truncate(s.parse_notes_len);
        self.here_doc_map.truncate(s.here_doc_map_len);
        if !self.pending_here_docs.ptr_eq(&s.pending_here_docs) {
            self.pending_here_docs = s.pending_here_docs.clone();
        }
    }

    /// `hereDocMap` as a map.
    pub fn here_doc_map(&self) -> BTreeMap<Id, Vec<Id>> {
        self.here_doc_map.iter().cloned().collect()
    }
}

/// `initialUserState`
pub fn initial_user_state() -> UserState {
    UserState::default()
}

pub fn code_for_parse_note(n: &ParseNote) -> Code {
    n.code
}

/// `IncompleteInterval SourcePos`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IncompleteInterval(pub SourcePos);

/// Store potential parse problems outside of parsec
#[derive(Clone, Debug, Default)]
pub struct SystemState {
    /// Innermost first, as in Haskell; shared on copy.
    pub context_stack: ContextStack,
    /// Oldest first.
    pub parse_problems: Vec<ParseNote>,
}

/// An O(1) copy of a `SystemState` that can restore it, given that the code
/// run in between only appends to `parse_problems` (all parse-problem writes
/// go through `add_parse_problem`, and every other restore returns to a
/// later or equal length).
pub struct SystemSnapshot {
    context_stack: ContextStack,
    parse_problems_len: usize,
}

impl SystemState {
    /// `Ms.get`
    pub fn save(&self) -> SystemSnapshot {
        SystemSnapshot { context_stack: self.context_stack.clone(), parse_problems_len: self.parse_problems.len() }
    }

    /// `Ms.put` of a state saved earlier.
    pub fn restore(&mut self, s: &SystemSnapshot) {
        debug_assert!(self.parse_problems.len() >= s.parse_problems_len);
        self.parse_problems.truncate(s.parse_problems_len);
        self.context_stack = s.context_stack.clone();
    }
}

/// `initialSystemState`
pub fn initial_system_state() -> SystemState {
    SystemState::default()
}

/// `data Environment m`
#[derive(Clone)]
pub struct Environment<'a> {
    pub system_interface: &'a dyn SystemInterface,
    pub check_sourced: bool,
    pub ignore_rc: bool,
    pub current_filename: String,
    pub shell_type_override: Option<Shell>,
}

/// Does this item on the context stack disable warnings for 'code'?
pub fn context_item_disables_code(also_check_sourced: bool, code: Code, item: &Context) -> bool {
    let disabling_ = |a: &Annotation| match a {
        Annotation::DisableComment(n, m) => code >= *n && code < *m,
        _ => false,
    };
    let disabling = |check_sourced: bool, item: &Context| match item {
        Context::ContextAnnotation(list) => list.iter().any(disabling_),
        Context::ContextSource(_) => !check_sourced,
        _ => false,
    };
    disabling(also_check_sourced, item)
}

impl<'a> P<'a> {
    /// DIVERGENCE: Haskell's `lastId` starts at `Id (-1)`; here that is `None`.
    pub fn get_last_id(&self) -> Option<Id> {
        self.user.ast.len().checked_sub(1).map(Id)
    }

    pub fn get_next_id_between(&mut self, start_pos: SourcePos, end_pos: SourcePos) -> Id {
        let id = self.user.ast.alloc(start_pos.pos(), end_pos.pos());
        self.user.span_files.push((start_pos.file, end_pos.file));
        id
    }

    pub fn get_next_id_spanning_tokens(&mut self, start_tok: Id, end_tok: Id) -> Id {
        let (start, _) = self.get_span_for_id(start_tok);
        let (_, end) = self.get_span_for_id(end_tok);
        self.get_next_id_between(start, end)
    }

    /// Get an ID starting from the first token of the list, and ending after the last
    pub fn get_next_id_spanning_token_list(&mut self, list: &[Id]) -> Id {
        match list {
            [] => {
                let pos = self.get_position();
                self.get_next_id_between(pos, pos)
            }
            [h, ..] => self.get_next_id_spanning_tokens(*h, *list.last().unwrap()),
        }
    }

    /// Get the span covered by an id
    pub fn get_span_for_id(&self, id: Id) -> (SourcePos, SourcePos) {
        let Some(&(sf, ef)) = self.user.span_files.get(id.0 as usize) else {
            panic!("{}", please_report("no parser span for id"));
        };
        let (s, e) = self.user.ast.span(id);
        (
            SourcePos { file: sf, line: s.line, column: s.column },
            SourcePos { file: ef, line: e.line, column: e.column },
        )
    }

    /// Create a new id with the same span as an existing one
    pub fn get_new_id_for(&mut self, id: Id) -> Id {
        let (s, e) = self.get_span_for_id(id);
        self.get_next_id_between(s, e)
    }

    pub fn start_span(&mut self) -> IncompleteInterval {
        IncompleteInterval(self.get_position())
    }

    pub fn end_span(&mut self, IncompleteInterval(start): IncompleteInterval) -> Id {
        let end_pos = self.get_position();
        self.get_next_id_between(start, end_pos)
    }

    /// DIVERGENCE: arena construction. Haskell builds `T_Foo id ...` as a
    /// value; here the token is stored at its (already allocated) id.
    pub fn node(&mut self, id: Id, t: Token) -> Id {
        self.user.ast.set(id, t);
        id
    }

    pub fn get_span_positions_for<T>(&mut self, m: impl FnOnce(&mut Self) -> PR<T>) -> PR<(SourcePos, SourcePos)> {
        let start = self.get_position();
        m(self)?;
        let end = self.get_position();
        Ok((start, end))
    }

    pub fn add_to_here_doc_map(&mut self, id: Id, list: Vec<Id>) {
        self.user.here_doc_map.push((id, list));
    }

    pub fn add_pending_here_doc(&mut self, id: Id, d: Dashed, q: Quoted, str: &str) {
        let context = self.get_current_contexts();
        self.user
            .pending_here_docs
            .push(HereDocContext::HereDocPending(id, d, q, str.to_string(), context));
    }

    /// Returns the pending here-docs oldest first (Haskell: `reverse`).
    pub fn pop_pending_here_docs(&mut self) -> Vec<HereDocContext> {
        let pending = std::mem::take(&mut self.user.pending_here_docs);
        let mut docs: Vec<HereDocContext> = pending.iter().cloned().collect();
        docs.reverse();
        docs
    }

    // DIVERGENCE: `getMap` is not ported; spans are read with `get_span_for_id`.

    /// Oldest first (the Haskell list is newest first).
    pub fn get_parse_notes(&self) -> &[ParseNote] {
        &self.user.parse_notes
    }

    pub fn add_parse_note(&mut self, n: ParseNote) {
        let irrelevant = self.should_ignore_code(code_for_parse_note(&n));
        if !irrelevant {
            self.user.parse_notes.push(n);
        }
    }

    pub fn ignore_problems_of<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let system_state = self.sys.save();
        let r = p(self)?;
        self.sys.restore(&system_state);
        Ok(r)
    }

    pub fn should_ignore_code(&self, code: Code) -> bool {
        let check_sourced = self.env.check_sourced;
        self.sys
            .context_stack
            .iter()
            .any(|c| context_item_disables_code(check_sourced, code, c))
    }

    pub fn get_current_annotations(&self, include_source: bool) -> Vec<Annotation> {
        let is_boundary = |c: &Context| match c {
            Context::ContextSource(_) => !include_source,
            _ => false,
        };
        self.sys
            .context_stack
            .iter()
            .take_while(|c| !is_boundary(c))
            .flat_map(|c| match c {
                Context::ContextAnnotation(list) => list.clone(),
                _ => vec![],
            })
            .collect()
    }

    pub fn should_follow(&mut self, file: &str) -> bool {
        let context = &self.sys.context_stack;
        let is_this_file = |c: &Context| matches!(c, Context::ContextSource(name) if name == file);
        let is_source = |c: &&Context| matches!(c, Context::ContextSource(_));
        if context.iter().any(is_this_file) {
            false
        } else if context.iter().filter(is_source).count() >= 100 {
            self.parse_problem(Severity::ErrorC, 1092, "Stopping at 100 'source' frames :O");
            false
        } else {
            true
        }
    }

    pub fn get_source_override(&self) -> Option<String> {
        let find_file = |c: &Context| match c {
            Context::ContextAnnotation(list) => list.iter().find_map(|a| match a {
                Annotation::SourceOverride(str) => Some(str.clone()),
                _ => None,
            }),
            _ => None,
        };
        let is_same_file = |c: &&Context| !matches!(c, Context::ContextSource(_));
        self.sys.context_stack.iter().take_while(is_same_file).find_map(find_file)
    }

    pub fn parse_problem(&mut self, level: Severity, code: Code, msg: &str) {
        let pos = self.get_position();
        self.parse_problem_at(pos, level, code, msg);
    }

    pub fn set_current_contexts(&mut self, c: ContextStack) {
        self.sys.context_stack = c;
    }

    /// Innermost first; O(1) (shared).
    pub fn get_current_contexts(&self) -> ContextStack {
        self.sys.context_stack.clone()
    }

    // DIVERGENCE: `popContext` returns the popped item, which no caller uses.
    pub fn pop_context(&mut self) {
        self.sys.context_stack.drop_head();
    }

    pub fn push_context(&mut self, c: Context) {
        self.sys.context_stack.push(c);
    }

    pub fn parse_problem_at_with_end(&mut self, start: SourcePos, end: SourcePos, level: Severity, code: Code, msg: &str) {
        let irrelevant = self.should_ignore_code(code);
        if !irrelevant {
            self.add_parse_problem(ParseNote::new(start, end, level, code, msg));
        }
    }

    pub fn add_parse_problem(&mut self, note: ParseNote) {
        self.sys.parse_problems.push(note);
    }

    pub fn parse_problem_at(&mut self, pos: SourcePos, level: Severity, code: Code, msg: &str) {
        self.parse_problem_at_with_end(pos, pos, level, code, msg);
    }

    pub fn parse_problem_at_id(&mut self, id: Id, level: Severity, code: Code, msg: &str) {
        let (start, end) = self.get_span_for_id(id);
        self.parse_problem_at_with_end(start, end, level, code, msg);
    }

    // Store non-parse problems inside

    pub fn parse_note(&mut self, c: Severity, l: Code, a: &str) {
        let pos = self.get_position();
        self.parse_note_at(pos, c, l, a);
    }

    pub fn parse_note_at(&mut self, pos: SourcePos, c: Severity, l: Code, a: &str) {
        self.add_parse_note(ParseNote::new(pos, pos, c, l, a));
    }

    pub fn parse_note_at_id(&mut self, id: Id, c: Severity, l: Code, a: &str) {
        let (start, end) = self.get_span_for_id(id);
        self.add_parse_note(ParseNote::new(start, end, c, l, a));
    }

    pub fn parse_note_at_with_end(&mut self, start: SourcePos, end: SourcePos, c: Severity, l: Code, a: &str) {
        self.add_parse_note(ParseNote::new(start, end, c, l, a));
    }

    //--------- Convenient combinators

    /// ``main `thenSkip` follow`` = `main <* optional follow`
    pub fn then_skip<T, U>(
        &mut self,
        main: impl FnOnce(&mut Self) -> PR<T>,
        follow: impl FnOnce(&mut Self) -> PR<U>,
    ) -> PR<T> {
        let r = main(self)?;
        self.optional(follow)?;
        Ok(r)
    }

    /// `unexpecting s p = try $ (try p >> fail ("Unexpected " ++ s)) <|> return ()`
    pub fn unexpecting<T>(&mut self, s: &str, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<()> {
        self.try_(|q| {
            q.alt(
                |q| {
                    q.try_(p)?;
                    q.fail(&format!("Unexpected {s}"))
                },
                |_| Ok(()),
            )
        })
    }

    /// `notFollowedBy2 = unexpecting ""`
    pub fn not_followed_by2<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<()> {
        self.unexpecting("", p)
    }

    /// `isFollowedBy p = (lookAhead . try $ p $> True) <|> return False`
    pub fn is_followed_by<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<bool> {
        self.alt(
            |q| q.look_ahead(|q| q.try_(|q| p(q).map(|_| true))),
            |_| Ok(false),
        )
    }

    /// `reluctantlyTill p end`
    pub fn reluctantly_till<T, E>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut end: impl FnMut(&mut Self) -> PR<E>,
    ) -> PR<Vec<T>> {
        // (lookAhead (void (try end) <|> eof) >> return [])
        //   <|> (do { x <- p; more <- reluctantlyTill p end; return (x:more) } <|> return [])
        //
        // DIVERGENCE: the recursion is unrolled into a loop (it recurses once
        // per character in e.g. readStringForParser). Once `p` has run, the
        // recursive call is simply the next step in sequence, and it can only
        // fail by consuming, so the loop has the same consumption, state and
        // error behaviour.
        let mut acc = Vec::new();
        loop {
            let step = self.alt(
                |q| {
                    q.look_ahead(|q| q.alt(|q| q.try_(&mut end).map(drop), |q| q.eof()))?;
                    Ok(None)
                },
                |q| q.alt(|q| p(q).map(Some), |_| Ok(None)),
            )?;
            match step {
                None => return Ok(acc),
                Some(x) => acc.push(x),
            }
        }
    }

    /// `reluctantlyTill1 p end`
    pub fn reluctantly_till1<T, E>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut end: impl FnMut(&mut Self) -> PR<E>,
    ) -> PR<Vec<T>> {
        self.not_followed_by2(&mut end)?;
        let x = p(self)?;
        let mut more = self.reluctantly_till(p, end)?;
        more.insert(0, x);
        Ok(more)
    }

    /// ``rest `attempting` branch`` = `(try branch >> rest) <|> rest`
    pub fn attempting<T, U>(
        &mut self,
        rest: impl FnMut(&mut Self) -> PR<T>,
        branch: impl FnOnce(&mut Self) -> PR<U>,
    ) -> PR<T> {
        let mut rest = rest;
        self.alt_with(
            &mut rest,
            |q, rest| {
                q.try_(branch)?;
                rest(q)
            },
            |q, rest| rest(q),
        )
    }

    /// ``parser `orFail` errorAction`` = `try parser <|> (errorAction >>= fail)`
    pub fn or_fail<T>(
        &mut self,
        parser: impl FnOnce(&mut Self) -> PR<T>,
        error_action: impl FnOnce(&mut Self) -> PR<String>,
    ) -> PR<T> {
        self.alt(
            |q| q.try_(parser),
            |q| {
                let msg = error_action(q)?;
                q.fail(&msg)
            },
        )
    }

    /// Construct a node with a parser, e.g. ``T_Literal `withParser` (readGenericLiteral ",")``.
    /// `node` receives the new id and the parser's result.
    pub fn with_parser<C, R>(
        &mut self,
        node: impl FnOnce(&mut Self, Id, C) -> R,
        parser: impl FnOnce(&mut Self) -> PR<C>,
    ) -> PR<R> {
        let start = self.start_span();
        let contents = parser(self)?;
        let id = self.end_span(start);
        Ok(node(self, id, contents))
    }

    /// `wasIncluded p = option False (p >> return True)`
    pub fn was_included<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<bool> {
        self.option(false, |q| p(q).map(|_| true))
    }

    pub fn accept_but_warn<T>(
        &mut self,
        parser: impl FnOnce(&mut Self) -> PR<T>,
        level: Severity,
        code: Code,
        note: &str,
    ) -> PR<()> {
        self.optional(|q| {
            q.try_(|q| {
                let pos = q.get_position();
                parser(q)?;
                q.parse_problem_at(pos, level, code, note);
                Ok(())
            })
        })
    }

    /// ``parsecBracket before after op = do { val <- before; op val `thenSkip` after val <|> (after val *> fail "") }``
    ///
    /// Note that `after` does not run when `op` fails after consuming input.
    pub fn parsec_bracket<V, T>(
        &mut self,
        before: impl FnOnce(&mut Self) -> PR<V>,
        after: impl FnOnce(&mut Self, V) -> PR<()>,
        op: impl FnOnce(&mut Self, &V) -> PR<T>,
    ) -> PR<T> {
        let val = before(self)?;
        let mut st = (Some(after), Some(val));
        self.alt_with(
            &mut st,
            |q, st| {
                let r = op(q, st.1.as_ref().expect("bracket value"))?;
                let (after, val) = (st.0.take().expect("bracket after"), st.1.take().expect("bracket value"));
                q.optional(|q| after(q, val))?;
                Ok(r)
            },
            |q, st| {
                let (after, val) = (st.0.take().expect("bracket after"), st.1.take().expect("bracket value"));
                after(q, val)?;
                q.fail("")
            },
        )
    }

    pub fn swap_context<T>(&mut self, contexts: ContextStack, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.parsec_bracket(
            |q| {
                let old = q.get_current_contexts();
                q.set_current_contexts(contexts);
                Ok(old)
            },
            |q, c| {
                q.set_current_contexts(c);
                Ok(())
            },
            |q, _| p(q),
        )
    }

    pub fn with_context<T>(&mut self, entry: Context, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.parsec_bracket(
            |q| {
                q.push_context(entry);
                Ok(())
            },
            |q, ()| {
                q.pop_context();
                Ok(())
            },
            |q, _| p(q),
        )
    }

    pub fn called<T>(&mut self, s: &'static str, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let pos = self.get_position();
        self.with_context(Context::ContextName(pos, s), p)
    }

    pub fn with_annotations<T>(&mut self, anns: Vec<Annotation>, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        if anns.is_empty() { p(self) } else { self.with_context(Context::ContextAnnotation(anns), p) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_is_a_shared_cons_list() {
        let mut a: PList<u32> = [1, 2, 3].into_iter().collect();
        assert_eq!(a.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        let b = a.clone();
        assert!(a.ptr_eq(&b));
        a.push(0);
        a.drop_head();
        a.drop_head();
        assert_eq!(a.iter().copied().collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(b.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(b.len(), 3);
        let mut e: PList<u32> = PList::new();
        e.drop_head();
        assert!(e.is_empty());
        // A long unshared list drops without recursing per node.
        let mut long = PList::new();
        for i in 0..1_000_000u32 {
            long.push(i);
        }
        drop(long);
    }

    #[test]
    fn system_snapshot_restores_problems_and_contexts() {
        let pos = SourcePos { file: FileId(0), line: 1, column: 1 };
        let mut sys = SystemState::default();
        sys.parse_problems.push(ParseNote::new(pos, pos, Severity::ErrorC, 1, "a"));
        let saved = sys.save();
        sys.context_stack.push(Context::ContextName(pos, "x"));
        sys.parse_problems.push(ParseNote::new(pos, pos, Severity::ErrorC, 2, "b"));
        sys.restore(&saved);
        assert_eq!(sys.parse_problems.len(), 1);
        assert!(sys.context_stack.is_empty());
    }
}
