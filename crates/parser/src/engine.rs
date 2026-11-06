// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964): the Parsec 3.1.16 engine it runs on
//
// DIVERGENCE: Parser.hs is a `ParsecT String UserState (ReaderT Environment
// (StateT SystemState m))`. Parsec is not available in Rust, so this module
// re-implements the parts of Parsec 3.1.16.1 (the version GHC 9.6 / stack
// lts-22.6 ships, matching ShellCheck.cabal's `parsec >= 3.1.14 && < 3.2`)
// that Parser.hs uses, with the same observable semantics:
//
// * A parser is a method or closure `FnOnce(&mut P) -> PR<T>`. Sequencing is
//   plain Rust `?`, which is Parsec's `>>=`.
// * Consumed vs. empty. Parsec's four continuations (cok, cerr, eok, eerr) are
//   reconstructed from a consumption counter: a combinator that catches errors
//   (`alt`, `try_`, `many`, `option`, `look_ahead`, ...) compares the counter
//   before and after running its argument. `Fail::Empty` returned by a
//   hand-written sequence may therefore be stale (the sequence consumed and
//   then a later step failed empty); every combinator reclassifies it, so only
//   combinators and primitives are guaranteed to report the right variant.
//   Hand-written code must not `match` on a `PR` to recover from failure; use a
//   combinator.
// * Backtracking. Wherever Parsec would continue with an earlier state `s`
//   (the second branch of `<|>`, the end of `many`, `lookAhead`, `try`), the
//   checkpoint taken there is restored: input, position, consumption counter,
//   and a clone-free snapshot of `UserState` (see `state::UserSnapshot`).
//   `SystemState` (`P::sys`) is never rolled back, exactly like the StateT
//   below ParsecT.
// * Errors. Only `fail` messages are kept (ShellCheck's `getStringFromParsec`
//   ignores Expect/UnExpect/SysUnExpect), plus a `hidden` flag recording
//   whether the Parsec error had any of those, because `mergeError` prefers
//   non-empty errors. Parsec carries an error along with every *empty*
//   success and merges it into what follows; `P::err` holds that carried error
//   and every primitive applies Parsec's `parserBind` merge rule to it.
// * `label`/`<?>` only adds Expect messages; ShellCheck hides `<?>`, and the
//   Expect text is never shown, so `label` is a no-op.
//
// DIVERGENCE: Parsec's `SourcePos` holds the file name as a String. Here it is
// a `FileId` into `P`'s interned file table so `SourcePos` stays `Copy`;
// comparisons that Parsec does on positions (`mergeError`) look the names up
// so the ordering is the same.
//
// DIVERGENCE: the input is an `Rc<[char]>` plus a cursor rather than a lazy
// `String`, because `subParse` and `tryWithErrors` switch to inputs created at
// parse time.

use std::cmp::Ordering;
use std::rc::Rc;

use shellcheck_core::ast::Pos;

use crate::state::{Environment, SystemState, UserState};

/// `panic!("unported: readFoo")`: placeholder body for a reader that a later
/// task ports.
#[macro_export]
macro_rules! unported {
    ($name:expr) => {
        panic!("unported: {}", $name)
    };
}

/// `a <|> b <|> c ...`, right-nested like Parsec's `infixr 1 <|>`.
/// `alt!(p, |p| ..., |p| ..., |p| ...)`; needs at least two alternatives.
#[macro_export]
macro_rules! alt {
    ($p:expr, $a:expr, $b:expr $(,)?) => {
        $p.alt($a, $b)
    };
    ($p:expr, $a:expr, $($rest:expr),+ $(,)?) => {
        $p.alt($a, |__p| $crate::alt!(__p, $($rest),+))
    };
}

/// Parsec's `choice [a, b, c]` = `a <|> (b <|> (c <|> mzero))`.
#[macro_export]
macro_rules! choice {
    ($p:expr, $a:expr $(,)?) => {
        $p.alt($a, |__p| __p.zero())
    };
    ($p:expr, $a:expr, $($rest:expr),+ $(,)?) => {
        $p.alt($a, |__p| $crate::choice!(__p, $($rest),+))
    };
}

/// Index into `P`'s table of file names (Parsec's `SourceName`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FileId(pub u32);

/// Parsec's `SourcePos`: file, 1-based line, 1-based column.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SourcePos {
    pub file: FileId,
    pub line: u32,
    pub column: u32,
}

impl SourcePos {
    /// The position without the file name, as stored in the `Ast` spans.
    pub fn pos(self) -> Pos {
        Pos { line: self.line, column: self.column }
    }
}

/// Parsec's `updatePosChar`: tabs advance to the next multiple of 8, plus 1.
pub fn update_pos_char(pos: SourcePos, c: char) -> SourcePos {
    match c {
        '\n' => SourcePos { line: pos.line + 1, column: 1, ..pos },
        '\t' => SourcePos { column: pos.column + 8 - ((pos.column - 1) % 8), ..pos },
        _ => SourcePos { column: pos.column + 1, ..pos },
    }
}

/// Parsec's `updatePosString`.
pub fn update_pos_string(pos: SourcePos, s: &str) -> SourcePos {
    s.chars().fold(pos, update_pos_char)
}

/// Parsec's `ParseError`, keeping only the `Message` texts (in order, empty
/// ones included). `hidden` is true when the Parsec error also held
/// SysUnExpect/UnExpect/Expect messages. To port `getStringFromParsec`, take
/// the last non-empty entry of `messages` (Parsec's `errorMessages` sorts
/// stably by message kind, and `Message` sorts last).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub pos: SourcePos,
    pub messages: Vec<String>,
    pub hidden: bool,
}

impl ParseError {
    /// `newErrorUnknown pos`
    pub fn unknown(pos: SourcePos) -> Self {
        ParseError { pos, messages: Vec::new(), hidden: false }
    }

    /// An error holding only a (Sys)UnExpect/Expect message.
    pub fn hidden(pos: SourcePos) -> Self {
        ParseError { pos, messages: Vec::new(), hidden: true }
    }

    /// `newErrorMessage (Message msg) pos`
    pub fn message(pos: SourcePos, msg: &str) -> Self {
        ParseError { pos, messages: vec![msg.to_string()], hidden: false }
    }

    /// `errorIsUnknown`
    pub fn is_unknown(&self) -> bool {
        self.messages.is_empty() && !self.hidden
    }
}

/// The two Parsec failure continuations: `eerr` and `cerr`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Fail {
    Empty(ParseError),
    Consumed(ParseError),
}

impl Fail {
    pub fn error(&self) -> &ParseError {
        match self {
            Fail::Empty(e) | Fail::Consumed(e) => e,
        }
    }

    pub fn into_error(self) -> ParseError {
        match self {
            Fail::Empty(e) | Fail::Consumed(e) => e,
        }
    }
}

pub type PR<T> = Result<T, Fail>;

/// The parser input: a shared buffer and a cursor (Parsec's `stateInput`).
#[derive(Clone, Debug)]
pub struct Input {
    buf: Rc<[char]>,
    at: usize,
}

impl Input {
    pub fn new(s: &str) -> Self {
        Input { buf: s.chars().collect::<Vec<_>>().into(), at: 0 }
    }

    /// The remaining characters.
    pub fn as_slice(&self) -> &[char] {
        &self.buf[self.at..]
    }

    pub fn is_empty(&self) -> bool {
        self.at >= self.buf.len()
    }

    pub fn as_string(&self) -> String {
        self.as_slice().iter().collect()
    }
}

/// Everything `P` restores when Parsec would continue from an earlier state.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    input: Input,
    pos: SourcePos,
    consumed: u64,
    user: crate::state::UserSnapshot,
}

/// The parser: Parsec's `State String UserState` plus the `ReaderT
/// Environment` and `StateT SystemState` layers below it.
pub struct P<'a> {
    input: Input,
    pos: SourcePos,
    /// Number of tokens consumed so far; rolled back with checkpoints.
    consumed: u64,
    /// The error Parsec carries along with the most recent success.
    err: ParseError,
    files: Vec<String>,
    pub user: UserState,
    pub sys: SystemState,
    pub env: Environment<'a>,
}

impl<'a> P<'a> {
    /// A parser at `initialPos filename` with `initialUserState` and
    /// `initialSystemState`, as `runParser` sets up.
    pub fn new(env: Environment<'a>, filename: &str, contents: &str) -> Self {
        let file = FileId(0);
        let pos = SourcePos { file, line: 1, column: 1 };
        P {
            input: Input::new(contents),
            pos,
            consumed: 0,
            err: ParseError::unknown(pos),
            files: vec![filename.to_string()],
            user: UserState::default(),
            sys: SystemState::default(),
            env,
        }
    }

    /// Run a parser to completion: Parsec's `runParserT` result.
    pub fn run<T>(&mut self, f: impl FnOnce(&mut Self) -> PR<T>) -> Result<T, ParseError> {
        f(self).map_err(Fail::into_error)
    }

    /// `lift $ runParserT p user name input`: run a separate Parsec parse on
    /// `input` starting at `initialPos name`, sharing `sys` and `env` (the
    /// monads below ParsecT) but nothing else. Returns the result and the
    /// final user state; the outer parse state is left untouched.
    pub fn run_parser_t<T>(
        &mut self,
        user: UserState,
        name: &str,
        input: Input,
        f: impl FnOnce(&mut Self) -> PR<T>,
    ) -> (Result<T, ParseError>, UserState) {
        let pos = self.initial_pos(name);
        let saved_input = std::mem::replace(&mut self.input, input);
        let saved_pos = std::mem::replace(&mut self.pos, pos);
        let saved_consumed = std::mem::replace(&mut self.consumed, 0);
        let saved_err = std::mem::replace(&mut self.err, ParseError::unknown(pos));
        let saved_user = std::mem::replace(&mut self.user, user);
        let r = f(self).map_err(Fail::into_error);
        let inner_user = std::mem::replace(&mut self.user, saved_user);
        self.input = saved_input;
        self.pos = saved_pos;
        self.consumed = saved_consumed;
        self.err = saved_err;
        self.touch();
        (r, inner_user)
    }

    // ---- files and positions -------------------------------------------

    /// Intern a file name.
    pub fn intern_file(&mut self, name: &str) -> FileId {
        match self.files.iter().position(|f| f == name) {
            Some(i) => FileId(i as u32),
            None => {
                self.files.push(name.to_string());
                FileId((self.files.len() - 1) as u32)
            }
        }
    }

    /// Parsec's `sourceName`.
    pub fn source_name(&self, pos: SourcePos) -> &str {
        &self.files[pos.file.0 as usize]
    }

    /// Parsec's `initialPos name`.
    pub fn initial_pos(&mut self, name: &str) -> SourcePos {
        SourcePos { file: self.intern_file(name), line: 1, column: 1 }
    }

    /// Parsec's derived `Ord SourcePos`: name, then line, then column.
    pub fn compare_pos(&self, a: SourcePos, b: SourcePos) -> Ordering {
        let names = if a.file == b.file {
            Ordering::Equal
        } else {
            self.source_name(a).cmp(self.source_name(b))
        };
        names.then(a.line.cmp(&b.line)).then(a.column.cmp(&b.column))
    }

    /// `getPosition`
    pub fn get_position(&mut self) -> SourcePos {
        self.touch();
        self.pos
    }

    /// `setPosition`
    pub fn set_position(&mut self, pos: SourcePos) {
        self.pos = pos;
        self.touch();
    }

    /// `getInput`
    pub fn get_input(&mut self) -> Input {
        self.touch();
        self.input.clone()
    }

    /// `setInput`
    pub fn set_input(&mut self, input: Input) {
        self.input = input;
        self.touch();
    }

    // ---- error bookkeeping ---------------------------------------------

    /// Parsec's `mergeError`.
    pub fn merge_error(&self, e1: ParseError, e2: ParseError) -> ParseError {
        if e2.is_unknown() && !e1.is_unknown() {
            return e1;
        }
        if e1.is_unknown() && !e2.is_unknown() {
            return e2;
        }
        match self.compare_pos(e1.pos, e2.pos) {
            Ordering::Equal => {
                let mut messages = e1.messages;
                messages.extend(e2.messages);
                ParseError { pos: e1.pos, messages, hidden: e1.hidden || e2.hidden }
            }
            Ordering::Greater => e1,
            Ordering::Less => e2,
        }
    }

    /// The merge `parserBind` does when its second half is empty: skipped
    /// when the first half's error is unknown.
    fn bind_merge(&self, prev: ParseError, own: ParseError) -> ParseError {
        if prev.is_unknown() { own } else { self.merge_error(prev, own) }
    }

    fn take_err(&mut self) -> ParseError {
        std::mem::replace(&mut self.err, ParseError::unknown(self.pos))
    }

    /// An empty success carrying `unknownError s` (getState, setPosition,
    /// lifted actions, ...).
    fn touch(&mut self) {
        if self.err.is_unknown() {
            self.err = ParseError::unknown(self.pos);
        }
    }

    /// An empty failure with error `own`, merged with the carried error.
    fn empty_fail(&mut self, own: ParseError) -> Fail {
        let prev = self.take_err();
        Fail::Empty(self.bind_merge(prev, own))
    }

    // ---- checkpoints ---------------------------------------------------

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            input: self.input.clone(),
            pos: self.pos,
            consumed: self.consumed,
            user: self.user.snapshot(),
        }
    }

    /// Return to a checkpoint. The carried error becomes unknown, as for a
    /// parser that is just starting.
    pub fn restore(&mut self, cp: &Checkpoint) {
        self.input = cp.input.clone();
        self.pos = cp.pos;
        self.consumed = cp.consumed;
        self.user.restore(&cp.user);
        self.err = ParseError::unknown(cp.pos);
    }

    /// Start a combinator: stash the carried error and take a checkpoint.
    fn begin(&mut self) -> (ParseError, Checkpoint) {
        let prev = self.take_err();
        (prev, self.checkpoint())
    }

    /// Run a sub-parser whose own errors start fresh. Returns whether it
    /// consumed input, and its own outcome.
    fn sub<T>(&mut self, start: u64, f: impl FnOnce(&mut Self) -> PR<T>) -> (bool, Result<T, ParseError>) {
        let r = f(self);
        let consumed = self.consumed != start || matches!(r, Err(Fail::Consumed(_)));
        (consumed, r.map_err(Fail::into_error))
    }

    /// Finish a combinator whose own outcome is `r`: apply `parserBind`'s
    /// merge with the error carried before it (`prev`).
    fn finish<T>(&mut self, prev: ParseError, consumed: bool, r: Result<T, ParseError>) -> PR<T> {
        match r {
            Ok(v) => {
                if !consumed {
                    let own = self.take_err();
                    self.err = self.bind_merge(prev, own);
                }
                Ok(v)
            }
            Err(e) if consumed => Err(Fail::Consumed(e)),
            Err(e) => Err(Fail::Empty(self.bind_merge(prev, e))),
        }
    }

    // ---- primitives ----------------------------------------------------

    fn peek(&self) -> Option<char> {
        self.input.as_slice().first().copied()
    }

    fn advance(&mut self, c: char) {
        self.input.at += 1;
        self.pos = update_pos_char(self.pos, c);
        self.consumed += 1;
    }

    /// `satisfy`
    pub fn satisfy(&mut self, f: impl FnOnce(char) -> bool) -> PR<char> {
        match self.peek() {
            Some(c) if f(c) => {
                self.advance(c);
                self.err = ParseError::unknown(self.pos);
                Ok(c)
            }
            _ => Err(self.empty_fail(ParseError::hidden(self.pos))),
        }
    }

    /// `anyToken`: consumes one char without moving the position.
    pub fn any_token(&mut self) -> PR<char> {
        match self.peek() {
            Some(c) => {
                self.input.at += 1;
                self.consumed += 1;
                self.err = ParseError::unknown(self.pos);
                Ok(c)
            }
            None => Err(self.empty_fail(ParseError::hidden(self.pos))),
        }
    }

    /// `anyChar`
    pub fn any_char(&mut self) -> PR<char> {
        self.satisfy(|_| true)
    }

    /// `char c`
    pub fn char_(&mut self, c: char) -> PR<char> {
        self.satisfy(|x| x == c)
    }

    /// `oneOf cs`
    pub fn one_of(&mut self, cs: &str) -> PR<char> {
        self.satisfy(|x| cs.contains(x))
    }

    /// `noneOf cs`
    pub fn none_of(&mut self, cs: &str) -> PR<char> {
        self.satisfy(|x| !cs.contains(x))
    }

    /// `string s` (Parsec 3.1.16 `tokens`): fails empty if the first char
    /// mismatches, and *consumed* if a later one does.
    pub fn string(&mut self, s: &str) -> PR<String> {
        let start = self.pos;
        let mut chars = s.chars();
        let Some(first) = chars.next() else {
            let own = ParseError::unknown(self.pos);
            let prev = self.take_err();
            self.err = self.bind_merge(prev, own);
            return Ok(String::new());
        };
        if self.peek() != Some(first) {
            return Err(self.empty_fail(ParseError::hidden(start)));
        }
        self.advance(first);
        for t in chars {
            if self.peek() != Some(t) {
                return Err(Fail::Consumed(ParseError::hidden(start)));
            }
            self.advance(t);
        }
        self.err = ParseError::unknown(self.pos);
        Ok(s.to_string())
    }

    /// `letter` (`isAlpha`)
    pub fn letter(&mut self) -> PR<char> {
        self.satisfy(is_alpha)
    }

    /// `digit` (`isDigit`: ASCII only)
    pub fn digit(&mut self) -> PR<char> {
        self.satisfy(|c| c.is_ascii_digit())
    }

    /// `upper` (`isUpper`)
    pub fn upper(&mut self) -> PR<char> {
        self.satisfy(is_upper)
    }

    /// `lower` (`isLower`)
    pub fn lower(&mut self) -> PR<char> {
        self.satisfy(is_lower)
    }

    /// `alphaNum` (`isAlphaNum`)
    pub fn alpha_num(&mut self) -> PR<char> {
        self.satisfy(|c| is_alpha(c) || c.is_numeric())
    }

    /// `space` (`isSpace`)
    pub fn space(&mut self) -> PR<char> {
        self.satisfy(is_space)
    }

    /// `spaces` = `skipMany space`
    pub fn spaces(&mut self) -> PR<()> {
        self.many(|p| p.space()).map(drop)
    }

    /// `fail msg`
    pub fn fail<T>(&mut self, msg: &str) -> PR<T> {
        Err(self.empty_fail(ParseError::message(self.pos, msg)))
    }

    /// `mzero` / `parserZero`
    pub fn zero<T>(&mut self) -> PR<T> {
        Err(self.empty_fail(ParseError::unknown(self.pos)))
    }

    /// `guard b`
    pub fn guard(&mut self, b: bool) -> PR<()> {
        if b { Ok(()) } else { self.zero() }
    }

    /// `unexpected msg` (the message text is never shown).
    fn unexpected<T>(&mut self) -> PR<T> {
        Err(self.empty_fail(ParseError::hidden(self.pos)))
    }

    /// `eof` = `notFollowedBy anyToken <?> "end of input"`
    pub fn eof(&mut self) -> PR<()> {
        self.not_followed_by(|p| p.any_token())
    }

    // ---- combinators ---------------------------------------------------

    /// `p <|> q`
    pub fn alt<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>, q: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.alt_with(&mut (), |s, _| p(s), |s, _| q(s))
    }

    /// `p <|> q` where both branches need mutable access to the same local
    /// state `st` (Rust closures cannot both borrow it mutably).
    pub fn alt_with<S, T>(
        &mut self,
        st: &mut S,
        p: impl FnOnce(&mut Self, &mut S) -> PR<T>,
        q: impl FnOnce(&mut Self, &mut S) -> PR<T>,
    ) -> PR<T> {
        let (prev, cp) = self.begin();
        let (c1, r1) = self.sub(cp.consumed, |s| p(s, st));
        let e1 = match r1 {
            _ if c1 => return self.finish(prev, true, r1),
            Ok(v) => return self.finish(prev, false, Ok(v)),
            Err(e1) => e1,
        };
        self.restore(&cp);
        let (c2, r2) = self.sub(cp.consumed, |s| q(s, st));
        match r2 {
            _ if c2 => self.finish(prev, true, r2),
            Ok(v) => {
                let own = self.take_err();
                self.err = self.merge_error(e1, own);
                self.finish(prev, false, Ok(v))
            }
            Err(e2) => {
                let e = self.merge_error(e1, e2);
                self.finish(prev, false, Err(e))
            }
        }
    }

    /// `choice (map f items)` = `foldr (<|>) mzero (map f items)`, e.g.
    /// `p.choice(["-o", "-a"], |p, s| p.try_(|p| p.string(s)))`. For a literal
    /// list of different parsers use the `choice!` macro.
    pub fn choice<I: IntoIterator, T>(&mut self, items: I, mut f: impl FnMut(&mut Self, I::Item) -> PR<T>) -> PR<T> {
        let mut it = items.into_iter();
        self.choice_go(&mut it, &mut f)
    }

    fn choice_go<I: Iterator, T>(&mut self, it: &mut I, f: &mut impl FnMut(&mut Self, I::Item) -> PR<T>) -> PR<T> {
        match it.next() {
            None => self.zero(),
            Some(x) => self.alt_with(&mut (it, f), |s, st| (st.1)(s, x), |s, st| s.choice_go(st.0, st.1)),
        }
    }

    /// `try p`
    pub fn try_<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let (prev, cp) = self.begin();
        let (c, r) = self.sub(cp.consumed, p);
        match r {
            Ok(v) => self.finish(prev, c, Ok(v)),
            Err(e) => {
                self.restore(&cp);
                self.finish(prev, false, Err(e))
            }
        }
    }

    /// `lookAhead p`
    pub fn look_ahead<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        let (prev, cp) = self.begin();
        let (c, r) = self.sub(cp.consumed, p);
        match r {
            Ok(v) => {
                self.restore(&cp);
                self.finish(prev, false, Ok(v))
            }
            Err(e) => self.finish(prev, c, Err(e)),
        }
    }

    /// `notFollowedBy p` = `try (do{ c <- try p; unexpected (show c) } <|> return ())`
    pub fn not_followed_by<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<()> {
        self.try_(|s| {
            s.alt(
                |s| {
                    s.try_(p)?;
                    s.unexpected()
                },
                |_| Ok(()),
            )
        })
    }

    /// `many p`
    pub fn many<T>(&mut self, mut p: impl FnMut(&mut Self) -> PR<T>) -> PR<Vec<T>> {
        let prev = self.take_err();
        let start = self.consumed;
        let mut acc = Vec::new();
        loop {
            self.err = ParseError::unknown(self.pos);
            let cp = self.checkpoint();
            let (c, r) = self.sub(cp.consumed, &mut p);
            match r {
                Ok(x) if c => acc.push(x),
                Ok(_) => panic!(
                    "Text.ParserCombinators.Parsec.Prim.many: combinator 'many' is applied to a parser that accepts an empty string."
                ),
                Err(e) if c => return Err(Fail::Consumed(e)),
                Err(e) => {
                    self.restore(&cp);
                    self.err = e;
                    let consumed = self.consumed != start;
                    return self.finish(prev, consumed, Ok(acc));
                }
            }
        }
    }

    /// `many1 p`
    pub fn many1<T>(&mut self, mut p: impl FnMut(&mut Self) -> PR<T>) -> PR<Vec<T>> {
        let x = p(self)?;
        let mut xs = self.many(p)?;
        xs.insert(0, x);
        Ok(xs)
    }

    /// `manyTill p end`
    pub fn many_till<T, E>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut end: impl FnMut(&mut Self) -> PR<E>,
    ) -> PR<Vec<T>> {
        // `scan = do{ _ <- end; return [] } <|> do{ x <- p; xs <- scan; return (x:xs) }`,
        // unrolled into a loop: once `p` has run, the rest of `scan` is in
        // sequence after it, which is what the loop does.
        let mut acc = Vec::new();
        loop {
            let step = self.alt(
                |s| {
                    end(s)?;
                    Ok(None)
                },
                |s| p(s).map(Some),
            )?;
            match step {
                None => return Ok(acc),
                Some(x) => acc.push(x),
            }
        }
    }

    /// `option x p`
    pub fn option<T>(&mut self, x: T, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.alt(p, |_| Ok(x))
    }

    /// `optionMaybe p`
    pub fn option_maybe<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<Option<T>> {
        self.option(None, |s| p(s).map(Some))
    }

    /// `optional p`
    pub fn optional<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>) -> PR<()> {
        self.alt(|s| p(s).map(drop), |_| Ok(()))
    }

    /// `sepBy1 p sep`
    pub fn sep_by1<T, S>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut sep: impl FnMut(&mut Self) -> PR<S>,
    ) -> PR<Vec<T>> {
        let x = p(self)?;
        let mut xs = self.many(|s| {
            sep(s)?;
            p(s)
        })?;
        xs.insert(0, x);
        Ok(xs)
    }

    /// `sepBy p sep`
    pub fn sep_by<T, S>(
        &mut self,
        p: impl FnMut(&mut Self) -> PR<T>,
        sep: impl FnMut(&mut Self) -> PR<S>,
    ) -> PR<Vec<T>> {
        self.alt(|s| s.sep_by1(p, sep), |_| Ok(Vec::new()))
    }

    /// `count n p`
    pub fn count<T>(&mut self, n: usize, mut p: impl FnMut(&mut Self) -> PR<T>) -> PR<Vec<T>> {
        let mut acc = Vec::with_capacity(n);
        for _ in 0..n {
            acc.push(p(self)?);
        }
        Ok(acc)
    }

    /// `chainl1 p op`, where `op` yields an `O` and `combine(o, x, y)` is
    /// Parsec's `f x y`.
    pub fn chainl1<T, O>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut op: impl FnMut(&mut Self) -> PR<O>,
        mut combine: impl FnMut(&mut Self, O, T, T) -> T,
    ) -> PR<T> {
        // `rest x = do{ f <- op; y <- p; rest (f x y) } <|> return x`,
        // unrolled into a loop (see `many_till`).
        let mut x = p(self)?;
        loop {
            let step = self.alt(
                |s| {
                    let f = op(s)?;
                    let y = p(s)?;
                    Ok(Some((f, y)))
                },
                |_| Ok(None),
            )?;
            match step {
                None => return Ok(x),
                Some((f, y)) => x = combine(self, f, x, y),
            }
        }
    }

    /// `chainr1 p op`, combining right to left.
    pub fn chainr1<T, O>(
        &mut self,
        mut p: impl FnMut(&mut Self) -> PR<T>,
        mut op: impl FnMut(&mut Self) -> PR<O>,
        mut combine: impl FnMut(&mut Self, O, T, T) -> T,
    ) -> PR<T> {
        // `scan = do{ x <- p; rest x }`,
        // `rest x = do{ f <- op; y <- scan; return (f x y) } <|> return x`.
        let first = p(self)?;
        let mut rest: Vec<(O, T)> = Vec::new();
        loop {
            let step = self.alt(
                |s| {
                    let f = op(s)?;
                    let y = p(s)?;
                    Ok(Some((f, y)))
                },
                |_| Ok(None),
            )?;
            match step {
                None => break,
                Some(pair) => rest.push(pair),
            }
        }
        let mut operands = vec![first];
        let mut ops = Vec::with_capacity(rest.len());
        for (f, y) in rest {
            ops.push(f);
            operands.push(y);
        }
        let mut acc = operands.pop().expect("chainr1 has an operand");
        while let Some(f) = ops.pop() {
            let x = operands.pop().expect("chainr1 operand per operator");
            acc = combine(self, f, x, acc);
        }
        Ok(acc)
    }

    /// `label p msg` / `p <?> msg`: Expect messages are never shown.
    pub fn label<T>(&mut self, p: impl FnOnce(&mut Self) -> PR<T>, _msg: &str) -> PR<T> {
        p(self)
    }
}

/// `Data.Char.isAlpha` (= `isLetter`: general categories L*).
// DIVERGENCE: std has no general-category lookup; `is_alphabetic` (the
// Alphabetic property) minus numerics differs from L* only for combining marks
// and a few symbols with Other_Alphabetic (e.g. U+0345, U+24B6).
pub fn is_alpha(c: char) -> bool {
    c.is_alphabetic() && !c.is_numeric()
}

/// `Data.Char.isUpper` (Lu or Lt).
// DIVERGENCE: approximated by the Uppercase property (Lu + Other_Uppercase);
// differs for titlecase letters and Other_Uppercase symbols.
pub fn is_upper(c: char) -> bool {
    c.is_uppercase()
}

/// `Data.Char.isLower` (Ll).
// DIVERGENCE: approximated by the Lowercase property (Ll + Other_Lowercase).
pub fn is_lower(c: char) -> bool {
    c.is_lowercase()
}

/// `Data.Char.isSpace`: `\t\n\v\f\r`, space, U+00A0, and category Zs.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t'..='\r'
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// Run `f` on a fresh thread with a `stack_bytes` stack, for callers that
/// parse deeply nested input (the readers are recursive, like the Haskell).
pub fn with_stack<R: Send>(stack_bytes: usize, f: impl FnOnce() -> R + Send) -> R {
    std::thread::scope(|s| {
        let handle = std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn_scoped(s, f)
            .expect("failed to spawn parser thread");
        handle.join().unwrap_or_else(|e| std::panic::resume_unwind(e))
    })
}
