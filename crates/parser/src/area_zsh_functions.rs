// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint extension, no ShellCheck counterpart. zsh function and
// group syntax. Every entry point is called only when `Environment::is_zsh`
// holds (the hook in the shared file checks it), so the other shells' parse
// paths are unchanged.
//
// Constructs (research Q3): C01 anonymous functions, C02 `&|` / `&!`, C03
// `=(cmd)`, C10 `{ } always { }`, C11 `}` without a `;` before it, C12 empty
// bodies, C13 multi-name functions, C14 odd function names, C15 empty
// `then`/`else`/`elif` bodies, C16 `if cond { }`, C22 `<(() { })` and C25
// anonymous functions with arguments and `&|`.
//
// AST mapping (no new `Token` variant):
// - `() { body } a b` is `T_BraceGroup([T_Function(ANON_FUNCTION_NAME, body),
//   call])`, where `call` is a `T_SimpleCommand` of that name and the
//   arguments. A function definition plus its immediate call, so the
//   analysis sees the body as a function scope that is called with the
//   arguments.
// - `&|` and `&!` are the `&` separator: `T_Backgrounded`.
// - `=(cmd)` is `T_ProcSub("=", cmd)`.
// - `{ A } always { B }` is `T_BraceGroup([{ A }, { B }])`.
// - `function a b { body }` is one `T_Function` per name; the body is parsed
//   again for each name (the checker drops the duplicate comments).
// - an empty body is an empty list.

use shelllint_core::ast::{FunctionKeyword, FunctionParentheses, Id, Token};
use shelllint_core::interface::Severity::ErrorC;

use crate::engine::{P, PR, SourcePos};

/// The name given to an anonymous function (zsh shows it as `(anon)` in `$funcstack`; that spelling would trip SC2288 as a command name).
pub(crate) const ANON_FUNCTION_NAME: &str = "zsh-anonymous-function";

/// Words that start another compound command: never the first name of a
/// `name() { }` definition.
const RESERVED: &[&str] = &[
    "if", "then", "elif", "else", "fi", "while", "until", "do", "done", "for", "select", "case", "esac", "time",
    "coproc", "function", "repeat", "foreach", "end", "nocorrect", "!",
];

/// The characters that end a word (so end the look-behind for `}`).
const WORD_ENDS: &str = ";|&<>()";

/// A function name the existing parser already reads (`function_chars` and
/// `extended_function_chars`); those are left to it.
fn is_plain_function_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "_#:+?-./^@,[]*=!".contains(c))
}

impl<'a> P<'a> {
    // ---- C02: `&|` and `&!` --------------------------------------------

    /// `&|` or `&!` as a command separator: a backgrounded command (the job
    /// is also disowned, which the analysis does not model).
    pub(crate) fn zsh_disown_separator(&mut self) -> PR<Option<(char, (SourcePos, SourcePos))>> {
        let start = self.get_position();
        let found = self.option_maybe(|p| {
            p.try_(|p| {
                p.char_('&')?;
                p.one_of("|!")?;
                // `a &|| b` is an error in zsh
                p.not_followed_by2(|p| p.char_('|'))
            })
        })?;
        if found.is_none() {
            return Ok(None);
        }
        let end = self.get_position();
        self.spacing()?;
        Ok(Some(('&', (start, end))))
    }

    // ---- C03 and C11: word parts ----------------------------------------

    /// Called first by `read_normal_word_part`. `=(cmd)` at the start of a
    /// part is a process-substitution-like expansion; an unquoted `}` that
    /// closes the enclosing group ends the word (`Err`).
    pub(crate) fn zsh_word_part(&mut self) -> PR<Option<Id>> {
        let input = self.get_input();
        let rest = input.as_slice();
        match rest.first() {
            Some('}') => {
                if self.zsh_closes_group(rest) {
                    return self.fail("Expected the closing } of the group");
                }
                Ok(None)
            }
            Some('=') if rest.get(1) == Some(&'(') => {
                let r = self.called("process substitution", |p| {
                    let start = p.start_span();
                    p.try_(|p| {
                        p.char_('=')?;
                        p.char_('(')
                    })?;
                    let list = p.read_compound_list_or_empty()?;
                    p.allspacing()?;
                    p.char_(')')?;
                    let id = p.end_span(start);
                    Ok(p.node(id, Token::T_ProcSub("=".to_string(), list)))
                })?;
                Ok(Some(r))
            }
            _ => Ok(None),
        }
    }

    /// Whether the `}` at the front of `rest` closes a group rather than
    /// being part of a word. zsh ends a word at an unquoted `}` unless the
    /// word opened a `{` of its own (`{a}`, `%{x%}`), and treats a `}` that
    /// starts a word and is glued to a word character (`}x`) as literal.
    fn zsh_closes_group(&self, rest: &[char]) -> bool {
        let before = self.consumed_chars();
        let mut i = before.len();
        let (mut open, mut close) = (0i32, 0i32);
        while i > 0 {
            let c = before[i - 1];
            if c.is_whitespace() || WORD_ENDS.contains(c) {
                break;
            }
            match c {
                '{' => open += 1,
                '}' => close += 1,
                _ => {}
            }
            i -= 1;
        }
        if open > close {
            return false;
        }
        let word_start = i == before.len();
        !(word_start && rest.get(1).is_some_and(|c| c.is_alphanumeric() || *c == '_'))
    }

    // ---- C10, C11, C12: brace groups ------------------------------------

    /// Whether a `{` at the front is a brace-expansion word (`{a,b}`), which
    /// is not a group.
    fn zsh_brace_expansion_ahead(&mut self) -> PR<bool> {
        self.is_followed_by(|p| p.read_braced())
    }

    /// `{ ... }` as zsh reads it: no space needed after `{`, any number of
    /// `;` or nothing at all inside, `}` allowed right after a command.
    fn zsh_brace_group_core(&mut self) -> PR<(Id, Vec<Id>)> {
        let start = self.start_span();
        self.char_('{')?;
        self.allspacing()?;
        // `{ }`, `{}`, `{ ; }`
        self.optional(|p| {
            p.try_(|p| {
                p.char_(';')?;
                p.allspacing()
            })
        })?;
        let empty = self.is_followed_by(|p| p.char_('}'))?;
        let list = if empty { Vec::new() } else { self.read_term()? };
        self.alt(
            |p| p.char_('}'),
            |p| {
                p.parse_problem(ErrorC, 1056, "Expected a '}'. If you have one, try a ; or \\n in front of it.");
                p.fail("Missing '}'")
            },
        )?;
        let id = self.end_span(start);
        self.spacing()?;
        let node = self.node(id, Token::T_BraceGroup(list.clone()));
        Ok((node, list))
    }

    /// `read_brace_group` under zsh.
    pub(crate) fn zsh_read_brace_group(&mut self) -> PR<Id> {
        self.called("brace group", |p| {
            let start = p.start_span();
            // `{echo,x}` is a brace-expansion word; checked before `{` is consumed
            if p.is_followed_by(|p| p.char_('{'))? && p.zsh_brace_expansion_ahead()? {
                return p.fail("Expected a brace group");
            }
            let (first, _) = p.zsh_brace_group_core()?;
            let has_always = p.is_followed_by(|p| {
                p.string("always")?;
                p.allspacing_or_fail()?;
                p.char_('{')
            })?;
            if !has_always {
                return Ok(first);
            }
            p.string("always")?;
            p.allspacing()?;
            let (second, _) = p.zsh_brace_group_core()?;
            // `{ A } always { B }`: A, then B, as two groups in sequence
            let a = p.zsh_wrap_command(first);
            let b = p.zsh_wrap_command(second);
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_BraceGroup(vec![a, b])))
        })
    }

    /// `T_Redirecting [] cmd`: how a compound command sits in a command list.
    fn zsh_wrap_command(&mut self, cmd: Id) -> Id {
        let id = self.get_new_id_for(cmd);
        self.node(id, Token::T_Redirecting(Vec::new(), cmd))
    }

    /// The body of a `{ }` that follows a condition (`if [[ ]] { }`,
    /// `while [[ ]] { }`), if there is one.
    pub(crate) fn zsh_try_brace_body(&mut self) -> PR<Option<Vec<Id>>> {
        let at_brace = self.is_followed_by(|p| {
            p.spacing()?;
            p.char_('{')
        })?;
        if !at_brace {
            return Ok(None);
        }
        self.spacing()?;
        let group = self.read_brace_group()?;
        match self.user.ast.get(group) {
            Token::T_BraceGroup(list) => Ok(Some(list.clone())),
            _ => Ok(None),
        }
    }

    /// Whether a `{` follows (after spaces on this line): the brace body of
    /// an `if`/`while`, not an unexpected parameter of a condition.
    pub(crate) fn zsh_lbrace_ahead(&mut self) -> bool {
        self.is_followed_by(|p| {
            p.spacing()?;
            p.char_('{')
        })
        .unwrap_or(false)
    }

    // ---- C15, C16: if/elif/else -----------------------------------------

    /// The `if` clause under zsh: `then` bodies may be empty, and
    /// `if cond { body }` needs no `then` and no `fi`.
    pub(crate) fn zsh_read_if_clause(&mut self) -> PR<Id> {
        self.called("if expression", |p| {
            let start = p.start_span();
            let pos = p.get_position();
            let (condition, action, first_brace) = p.zsh_if_part("if", pos)?;
            let elifs = p.many(|p| p.zsh_elif_part())?;
            let mut brace = first_brace || elifs.iter().any(|e| e.2);
            let else_part = p.option(None, |p| p.zsh_else_part(brace).map(Some))?;
            let elses = match else_part {
                Some((body, else_brace)) => {
                    brace = brace || else_brace;
                    body
                }
                None => vec![],
            };
            if !brace {
                p.or_fail(
                    |p| p.g_fi(),
                    |p| {
                        p.parse_problem_at(pos, ErrorC, 1046, "Couldn't find 'fi' for this 'if'.");
                        p.parse_problem(ErrorC, 1047, "Expected 'fi' matching previously mentioned 'if'.");
                        Ok("Expected 'fi'".to_string())
                    },
                )?;
            }
            let id = p.end_span(start);
            let mut parts = vec![(condition, action)];
            parts.extend(elifs.into_iter().map(|(c, a, _)| (c, a)));
            Ok(p.node(id, Token::T_IfExpression(parts, elses)))
        })
    }

    /// An `if` or `elif` part: (condition, action, written with braces).
    fn zsh_if_part(&mut self, kw: &str, pos: SourcePos) -> PR<(Vec<Id>, Vec<Id>, bool)> {
        if kw == "if" {
            self.g_if()?;
        } else {
            self.g_elif()?;
        }
        self.allspacing()?;
        let condition = self.read_term()?;
        if let Some(body) = self.zsh_try_brace_body()? {
            return Ok((condition, body, true));
        }
        self.if_next_token(
            |p| crate::alt!(p, |p| p.g_fi(), |p| p.g_elif(), |p| p.g_else()),
            |p| {
                p.parse_problem_at(pos, ErrorC, 1049, &format!("Did you forget the 'then' for this '{kw}'?"));
                Ok(())
            },
        )?;
        self.called("then clause", |p| {
            p.or_fail(
                |p| p.g_then(),
                |p| {
                    p.parse_problem(ErrorC, 1050, "Expected 'then'.");
                    Ok("Expected 'then'".to_string())
                },
            )?;
            // zsh accepts `then;`
            p.optional(|p| p.g_semi())?;
            p.allspacing()?;
            let action = p.zsh_term_or_empty()?;
            Ok((condition, action, false))
        })
    }

    fn zsh_elif_part(&mut self) -> PR<(Vec<Id>, Vec<Id>, bool)> {
        self.called("elif clause", |p| {
            let pos = p.get_position();
            p.zsh_if_part("elif", pos)
        })
    }

    /// The `else` part: (body, written with braces).
    fn zsh_else_part(&mut self, brace_if: bool) -> PR<(Vec<Id>, bool)> {
        self.called("else clause", |p| {
            let pos = p.get_position();
            p.g_else()?;
            if brace_if && let Some(body) = p.zsh_try_brace_body()? {
                return Ok((body, true));
            }
            p.optional(|p| {
                p.try_(|p| p.look_ahead(|p| p.g_if()))?;
                p.parse_problem_at(
                    pos,
                    ErrorC,
                    1075,
                    "Use 'elif' instead of 'else if' (or put 'if' on new line if nesting).",
                );
                Ok(())
            })?;
            p.optional(|p| p.g_semi())?;
            p.allspacing()?;
            Ok((p.zsh_term_or_empty()?, false))
        })
    }

    /// A command list, or nothing when `fi`, `elif` or `else` comes next.
    fn zsh_term_or_empty(&mut self) -> PR<Vec<Id>> {
        let empty = self.is_followed_by(|p| crate::alt!(p, |p| p.g_fi(), |p| p.g_elif(), |p| p.g_else()))?;
        if empty { Ok(Vec::new()) } else { self.read_term() }
    }

    // ---- C01, C25 (and C22): anonymous functions ------------------------

    /// `() { body } args`, `function { body } args`, and `function () { }`.
    /// `None` (nothing consumed) when the text is not one of those.
    pub(crate) fn zsh_anonymous_function(&mut self) -> PR<Option<Id>> {
        let input = self.get_input();
        let rest = input.as_slice();
        let starts = rest.starts_with(&['(', ')'])
            || (rest.starts_with(&['f', 'u', 'n', 'c', 't', 'i', 'o', 'n'])
                && rest.get(8).is_some_and(|c| c.is_whitespace() || *c == '('));
        if !starts {
            return Ok(None);
        }
        self.option_maybe(|p| p.try_(|p| p.zsh_read_anonymous_function()))
    }

    fn zsh_read_anonymous_function(&mut self) -> PR<Id> {
        let start = self.start_span();
        let (keyword, parens) = self.alt(
            |p| {
                p.string("()")?;
                Ok((false, true))
            },
            |p| {
                p.string("function")?;
                p.allspacing()?;
                let parens = p.option(false, |p| {
                    p.string("()")?;
                    Ok(true)
                })?;
                Ok((true, parens))
            },
        )?;
        self.allspacing()?;
        self.look_ahead(|p| p.one_of("{("))?;
        let body = self.alt(|p| p.read_brace_group(), |p| p.read_subshell())?;
        let fn_id = self.end_span(start);
        let function = self.node(
            fn_id,
            Token::T_Function(
                FunctionKeyword(keyword),
                FunctionParentheses(parens),
                ANON_FUNCTION_NAME.to_string(),
                body,
            ),
        );
        let definition = self.zsh_wrap_command(function);

        // the immediate call: arguments and redirections up to the end of the line
        let suffix = self.option(Vec::new(), |p| p.read_cmd_suffix())?;
        let name_part = self.get_new_id_for(fn_id);
        self.node(name_part, Token::T_Literal(ANON_FUNCTION_NAME.to_string()));
        let name_id = self.get_new_id_for(fn_id);
        let name = self.node(name_id, Token::T_NormalWord(vec![name_part]));
        let call = self.zsh_simple_command(name, &suffix);

        let id = self.end_span(start);
        Ok(self.node(id, Token::T_BraceGroup(vec![definition, call])))
    }

    /// `T_Redirecting redirs (T_SimpleCommand [] (cmd : args))`, splitting the
    /// suffix into redirections and arguments as `make_simple_command` does.
    fn zsh_simple_command(&mut self, cmd: Id, suffix: &[Id]) -> Id {
        let (redirs, args): (Vec<Id>, Vec<Id>) =
            suffix.iter().copied().partition(|t| matches!(self.user.ast.get(*t), Token::T_FdRedirect(..)));
        let mut all = vec![cmd];
        all.extend(args);
        let id1 = self.get_next_id_spanning_token_list(&[vec![cmd], suffix.to_vec()].concat());
        let id2 = self.get_new_id_for(id1);
        self.node(id2, Token::T_SimpleCommand(Vec::new(), all));
        self.node(id1, Token::T_Redirecting(redirs, id2))
    }

    // ---- C13, C14: multi-name and odd function names --------------------

    /// `function a b { }`, `a b() { }` and names with expansions or other
    /// characters the plain reader rejects. `None` (nothing consumed) for a
    /// plain single name, which the existing reader handles.
    pub(crate) fn zsh_function_definition(&mut self) -> PR<Option<Id>> {
        let input = self.get_input();
        let rest = input.as_slice();
        let line: &[char] = &rest[..rest.iter().position(|c| *c == '\n').unwrap_or(rest.len())];
        let keyword_form = rest.starts_with(&['f', 'u', 'n', 'c', 't', 'i', 'o', 'n'])
            && rest.get(8).is_some_and(|c| *c == ' ' || *c == '\t');
        let has_parens = line.windows(2).any(|w| w == ['(', ')']);
        if !keyword_form && !has_parens {
            return Ok(None);
        }
        self.option_maybe(|p| p.try_(|p| p.zsh_read_function_definition(keyword_form)))
    }

    fn zsh_read_function_definition(&mut self, keyword_form: bool) -> PR<Id> {
        let start = self.start_span();
        if keyword_form {
            self.string("function")?;
            self.spacing1()?;
        }
        let mut names: Vec<String> = Vec::new();
        loop {
            if self.is_followed_by(|p| p.char_('('))? {
                break;
            }
            if keyword_form
                && self.is_followed_by(|p| p.char_('{'))?
                && !self.is_followed_by(|p| p.read_braced())?
                && !names.is_empty()
            {
                break;
            }
            let before = self.get_input();
            // the name is a word; its expansions are not part of the tree
            self.ignore_problems_of(|p| p.read_normal_word())?;
            let after = self.get_input();
            let n = before.as_slice().len() - after.as_slice().len();
            let name: String = before.as_slice()[..n].iter().collect();
            self.guard(!names.is_empty() || !RESERVED.contains(&name.as_str()))?;
            // without `function`, a brace expansion is not a name
            self.guard(keyword_form || !name.starts_with('{'))?;
            names.push(name);
            if keyword_form { self.allspacing()? } else { self.spacing()? };
        }
        let parens = self.option(false, |p| {
            p.char_('(')?;
            p.spacing()?;
            p.char_(')')?;
            Ok(true)
        })?;
        self.guard(!names.is_empty() && (keyword_form || parens))?;
        // a single plain name is the existing reader's
        self.guard(names.len() > 1 || !is_plain_function_name(&names[0]))?;
        self.allspacing()?;
        self.look_ahead(|p| p.one_of("{("))?;
        let (body_input, body_pos) = (self.get_input(), self.get_position());
        let body = self.alt(|p| p.read_brace_group(), |p| p.read_subshell())?;
        let id = self.end_span(start);

        let kw = FunctionKeyword(keyword_form);
        let pa = FunctionParentheses(parens);
        let first = self.node(id, Token::T_Function(kw.clone(), pa.clone(), names[0].clone(), body));
        if names.len() == 1 {
            return Ok(first);
        }
        // `function a b { body }` defines b with the same body: read it again
        let end = (self.get_input(), self.get_position());
        let mut defs = vec![self.zsh_wrap_command(first)];
        for other in &names[1..] {
            let again = self.ignore_problems_of(|p| {
                p.set_input(body_input.clone());
                p.set_position(body_pos);
                p.alt(|p| p.read_brace_group(), |p| p.read_subshell())
            })?;
            let fid = self.get_new_id_for(id);
            let f = self.node(fid, Token::T_Function(kw.clone(), pa.clone(), other.clone(), again));
            defs.push(self.zsh_wrap_command(f));
        }
        self.set_input(end.0);
        self.set_position(end.1);
        let group = self.end_span(start);
        Ok(self.node(group, Token::T_BraceGroup(defs)))
    }
}
