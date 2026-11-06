// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 156-463:
// parse state, ids and spans, here-doc bookkeeping, parse notes and problems,
// the context stack, and the convenience combinators.
//
// DIVERGENCE: Haskell lists that are only ever prepended to (`parseNotes`,
// `parseProblems`, `pendingHereDocs`) are stored oldest-first, so backtracking
// can truncate them; the Haskell order is the reverse. `contextStack` keeps
// the Haskell order (index 0 is the innermost context).
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Context {
    ContextName(SourcePos, String),
    ContextAnnotation(Vec<Annotation>),
    ContextSource(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HereDocContext {
    /// on linefeed, read this T_HereDoc
    HereDocPending(Id, Dashed, Quoted, String, Vec<Context>),
}

/// `data UserState`: everything that rolls back on backtracking.
#[derive(Clone, Debug, Default)]
pub struct UserState {
    /// The token arena; also `positionMap` (spans) and `lastId` (length).
    pub ast: Ast,
    /// File of each span's (start, end); parallel to `ast`.
    span_files: Vec<(FileId, FileId)>,
    /// Oldest first.
    pub parse_notes: Vec<ParseNote>,
    /// `Map.insert` log, oldest first; later entries win.
    pub here_doc_map: Vec<(Id, Vec<Id>)>,
    /// Oldest first (the order `popPendingHereDocs` returns).
    pub pending_here_docs: Vec<HereDocContext>,
}

/// What a checkpoint needs to restore a `UserState`. Everything but the
/// pending here-docs (usually empty) is a length to truncate back to.
#[derive(Clone, Debug)]
pub struct UserSnapshot {
    ast_len: u32,
    parse_notes_len: usize,
    here_doc_map_len: usize,
    pending_here_docs: Vec<HereDocContext>,
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
        if self.pending_here_docs != s.pending_here_docs {
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
    /// Innermost first, as in Haskell.
    pub context_stack: Vec<Context>,
    /// Oldest first.
    pub parse_problems: Vec<ParseNote>,
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
        std::mem::take(&mut self.user.pending_here_docs)
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
        let system_state = self.sys.clone();
        let r = p(self)?;
        self.sys = system_state;
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

    pub fn set_current_contexts(&mut self, c: Vec<Context>) {
        self.sys.context_stack = c;
    }

    /// Innermost first.
    pub fn get_current_contexts(&self) -> Vec<Context> {
        self.sys.context_stack.clone()
    }

    pub fn pop_context(&mut self) -> Option<Context> {
        if self.sys.context_stack.is_empty() {
            None
        } else {
            Some(self.sys.context_stack.remove(0))
        }
    }

    pub fn push_context(&mut self, c: Context) {
        self.sys.context_stack.insert(0, c);
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

    pub fn swap_context<T>(&mut self, contexts: Vec<Context>, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
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

    pub fn called<T>(&mut self, s: &str, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let pos = self.get_position();
        self.with_context(Context::ContextName(pos, s.to_string()), p)
    }

    pub fn with_annotations<T>(&mut self, anns: Vec<Annotation>, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        if anns.is_empty() { p(self) } else { self.with_context(Context::ContextAnnotation(anns), p) }
    }
}
