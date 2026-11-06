// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2054-2438:
// separators, simple commands, `source`, pipelines, and/or lists, terms.

use shellcheck_core::astlib::{get_literal_string, get_word_parts, is_string_expansion};
use shellcheck_core::ast::{Annotation, Ast, Id, Token};
use shellcheck_core::interface::Severity::{ErrorC, InfoC, WarningC};

use crate::engine::{P, PR, SourcePos, is_alpha};
use crate::state::Context;
use crate::{alt, choice};

/// The suffix reader `readSimpleCommand` picks for a command name.
type SuffixReader<'a> = fn(&mut P<'a>) -> PR<Vec<Id>>;

impl<'a> P<'a> {
    pub(crate) fn read_newline_list(&mut self) -> PR<String> {
        let r = self.many1(|p| p.then_skip(|p| p.alt(|p| p.linefeed(), |p| p.carriage_return()), |p| p.spacing()))?;
        // checkBadBreak
        self.optional(|p| {
            let pos = p.get_position();
            p.try_(|p| p.look_ahead(|p| p.one_of("|&")))?; //  See if the next thing could be |, || or &&
            p.not_followed_by2(|p| p.string("&>"))?; //  Except &> or &>> which is valid
            p.parse_problem_at(
                pos,
                ErrorC,
                1133,
                "Unexpected start of line. If breaking lines, |/||/&& should be at the end of the previous one.",
            );
            Ok(())
        })?;
        Ok(r.into_iter().collect())
    }

    pub(crate) fn read_line_break(&mut self) -> PR<()> {
        self.optional(|p| p.read_newline_list())
    }

    /// The separator char and its (start, end) span.
    pub(crate) fn read_separator_op(&mut self) -> PR<(char, (SourcePos, SourcePos))> {
        self.not_followed_by2(|p| p.alt(|p| p.g_and_if().map(drop), |p| p.read_case_separator().map(drop)))?;
        self.not_followed_by2(|p| p.string("&>"))?;
        let start = self.get_position();
        let f = alt!(
            self,
            |p: &mut Self| p.try_(|p| {
                let pos = p.get_position();
                p.char_('&')?;
                p.optional(|p| {
                    choice!(
                        p,
                        |p: &mut Self| {
                            p.look_ahead(|p| p.choice(["amp;", "gt;", "lt;"], |p, s| p.try_(|p| p.string(s))))?;
                            p.parse_problem_at(
                                pos,
                                ErrorC,
                                1109,
                                "This is an unquoted HTML entity. Replace with corresponding character.",
                            );
                            Ok(())
                        },
                        |p: &mut Self| {
                            p.try_(|p| p.look_ahead(|p| p.variable_start()))?;
                            p.parse_problem_at(
                                pos,
                                WarningC,
                                1132,
                                "This & terminates the command. Escape it or add space after & to silence.",
                            );
                            Ok(())
                        },
                    )
                })?;

                p.spacing()?;
                let pos = p.get_position();
                p.char_(';')?;
                // In case statements we might have foo & ;;
                p.not_followed_by2(|p| p.char_(';'))?;
                p.parse_problem_at(pos, ErrorC, 1045, "It's not 'foo &; bar', just 'foo & bar'.");
                Ok('&')
            }),
            |p: &mut Self| p.char_(';'),
            |p: &mut Self| p.char_('&'),
        )?;
        let end = self.get_position();
        self.spacing()?;
        Ok((f, (start, end)))
    }

    pub(crate) fn read_sequential_sep(&mut self) -> PR<()> {
        self.alt(
            |p| {
                p.g_semi()?;
                p.read_line_break()
            },
            |p| p.read_newline_list().map(drop),
        )
    }

    pub(crate) fn read_separator(&mut self) -> PR<(char, (SourcePos, SourcePos))> {
        self.alt(
            |p| {
                let separator = p.read_separator_op()?;
                p.read_line_break()?;
                Ok(separator)
            },
            |p| {
                let start = p.get_position();
                p.read_newline_list()?;
                let end = p.get_position();
                Ok(('\n', (start, end)))
            },
        )
    }

    pub(crate) fn read_simple_command(&mut self) -> PR<Id> {
        self.called("simple command", |p| {
            let prefix = p.option(Vec::new(), |p| p.read_cmd_prefix())?;
            p.skip_annotation_and_warn()?;
            let cmd = p.option(None, |p| p.read_cmd_name().map(Some))?;
            if prefix.is_empty() && cmd.is_none() {
                return p.fail("Expected a command");
            }

            match cmd {
                None => {
                    let id1 = p.get_next_id_spanning_token_list(&prefix);
                    let id2 = p.get_new_id_for(id1);
                    Ok(p.make_simple_command(id1, id2, &prefix, &[], &[]))
                }

                Some(cmd) => {
                    p.validate_command(cmd);
                    // We have to ignore possible parsing problems from the lookAhead parser
                    // DIVERGENCE: the lookahead's tokens are rolled back out of
                    // the arena, so the only thing `isCommand` needs from the
                    // first argument (its literal name) is extracted inside it.
                    let first_argument = p.ignore_problems_of(|p| {
                        p.option_maybe(|p| {
                            p.try_(|p| {
                                p.look_ahead(|p| {
                                    let w = p.read_cmd_word()?;
                                    Ok(p.command_literal(w))
                                })
                            })
                        })
                    })?;
                    let cmd_literal = p.command_literal(cmd);
                    // If `export` or other modifier commands are called with `builtin` we have to look at the first argument
                    let chosen = if is_command(&cmd_literal, &["builtin"]) {
                        first_argument.unwrap_or_else(|| cmd_literal.clone())
                    } else {
                        cmd_literal.clone()
                    };
                    let table: [(&[&str], SuffixReader<'a>); 4] = [
                        (&["declare", "export", "local", "readonly", "typeset"], Self::read_modifier_suffix),
                        (&["time"], Self::read_time_suffix),
                        (&["let"], Self::read_let_suffix),
                        (&["eval"], Self::read_eval_suffix),
                    ];
                    let parser = get_parser(Self::read_cmd_suffix, &chosen, &table);
                    let suffix = p.option(Vec::new(), parser)?;

                    let mut all = prefix.clone();
                    all.push(cmd);
                    all.extend_from_slice(&suffix);
                    let id1 = p.get_next_id_spanning_token_list(&all);
                    let id2 = p.get_new_id_for(id1);

                    let result = p.make_simple_command(id1, id2, &prefix, &[cmd], &suffix);
                    if is_command(&cmd_literal, &["source", "."]) {
                        p.read_source(result)
                    } else if is_command(&cmd_literal, &["trap"]) {
                        p.syntax_check_trap(result)?;
                        Ok(result)
                    } else {
                        Ok(result)
                    }
                }
            }
        })
    }

    /// `isCommand`'s view of a token: the `s` of `T_NormalWord _ [T_Literal _ s]`.
    fn command_literal(&self, t: Id) -> Option<String> {
        let ast = &self.user.ast;
        match ast.get(t) {
            Token::T_NormalWord(parts) => match parts.as_slice() {
                [x] => match ast.get(*x) {
                    Token::T_Literal(s) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn validate_command(&mut self, cmd: Id) {
        let ast = &self.user.ast;
        let lit = |x: Id| match ast.get(x) {
            Token::T_Literal(s) => Some(s.clone()),
            _ => None,
        };
        let glob = |x: Id| match ast.get(x) {
            Token::T_Glob(s) => Some(s.clone()),
            _ => None,
        };
        let parts = match ast.get(cmd) {
            Token::T_NormalWord(parts) => parts.clone(),
            _ => return,
        };
        if parts.len() == 1 && lit(parts[0]).as_deref() == Some("//") {
            self.comment_warning(cmd);
        } else if parts.len() >= 2 && lit(parts[0]).as_deref() == Some("/") && glob(parts[1]).as_deref() == Some("*") {
            self.comment_warning(cmd);
        } else if let Some(str) = parts.first().and_then(|&x| lit(x)) {
            // DIVERGENCE: Haskell's `toLower` is the simple case mapping; the
            // only char whose full lowercase is longer (U+0130) simply maps to
            // the first char of it, so the first char is taken.
            let cmd_string: String = str
                .chars()
                .take_while(|&c| is_alpha(c))
                .map(|c| c.to_lowercase().next().unwrap_or(c))
                .collect();
            if cmd_string == "elsif" || cmd_string == "elseif" {
                self.parse_problem_at_id(cmd, ErrorC, 1131, "Use 'elif' to start another branch.");
            }
        }
    }

    fn syntax_check_trap(&mut self, cmd: Id) -> PR<()> {
        let ast = &self.user.ast;
        let arg = match ast.get(cmd) {
            Token::T_Redirecting(_, sc) => match ast.get(*sc) {
                Token::T_SimpleCommand(_, words) if words.len() >= 2 => words[1],
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };
        // checkArg
        match get_literal_string(ast, arg) {
            None => Ok(()),
            Some(str) if str.starts_with('-') => Ok(()),
            Some(str) => {
                let (start, _end) = self.get_span_for_id(arg);
                self.sub_parse(
                    start,
                    |p| {
                        p.alt(
                            |p| {
                                p.try_with_errors(|p| {
                                    p.read_compound_list_or_empty()?;
                                    p.verify_eof()
                                })
                            },
                            |_| Ok(()),
                        )
                    },
                    &str,
                )
            }
        }
    }

    fn comment_warning(&mut self, id: Id) {
        self.parse_problem_at_id(id, ErrorC, 1127, "Was this intended as a comment? Use # in sh.");
    }

    /// Stores `T_Redirecting id1 redirs $ T_SimpleCommand id2 assigns args`
    /// and returns `id1`.
    fn make_simple_command(&mut self, id1: Id, id2: Id, prefix: &[Id], cmd: &[Id], suffix: &[Id]) -> Id {
        let ast = &self.user.ast;
        let assignment = |t: &Id| matches!(ast.get(*t), Token::T_Assignment(..));
        let redirection = |t: &Id| matches!(ast.get(*t), Token::T_FdRedirect(..));
        let (pre_assigned, pre_rest): (Vec<Id>, Vec<Id>) = prefix.iter().copied().partition(assignment);
        let (pre_redirected, pre_rest2): (Vec<Id>, Vec<Id>) = pre_rest.iter().copied().partition(redirection);
        let (post_redirected, post_rest): (Vec<Id>, Vec<Id>) = suffix.iter().copied().partition(redirection);

        let redirs = [pre_redirected, post_redirected].concat();
        let assigns = pre_assigned;
        let args = [cmd.to_vec(), pre_rest2, post_rest].concat();
        self.node(id2, Token::T_SimpleCommand(assigns, args));
        self.node(id1, Token::T_Redirecting(redirs, id2))
    }

    pub(crate) fn read_source(&mut self, t: Id) -> PR<Id> {
        let ast = &self.user.ast;
        let (cmd_id, cmd, args) = match ast.get(t) {
            Token::T_Redirecting(_, sc) => match ast.get(*sc) {
                Token::T_SimpleCommand(_, words) if !words.is_empty() => (*sc, words[0], words[1..].to_vec()),
                _ => return Ok(t),
            },
            _ => return Ok(t),
        };
        let file = get_file(ast, &args);
        let override_ = self.get_source_override();
        let ast = &self.user.ast;
        let literal_file = override_
            .or_else(|| file.and_then(|f| get_literal_string(ast, f)))
            .or_else(|| file.and_then(|f| strip_dynamic_prefix(ast, f)))
            // Hack to avoid 'source ~/foo' trying to read from literal tilde
            .filter(|name| !name.starts_with("~/"));
        let file_id = file.unwrap_or(cmd);
        let Some(filename) = literal_file else {
            self.parse_note_at_id(
                file_id,
                WarningC,
                1090,
                "ShellCheck can't follow non-constant source. Use a directive to specify location.",
            );
            return Ok(t);
        };
        let proceed = self.should_follow(&filename);
        if !proceed {
            // FIXME: This actually gets squashed without -a
            self.parse_note_at_id(file_id, InfoC, 1093, "This file appears to be recursively sourced. Ignoring.");
            return Ok(t);
        }
        let sys = self.env.system_interface;
        let (input, resolved_file) = if filename == "/dev/null" {
            // always allow /dev/null
            (Ok(String::new()), filename)
        } else {
            let all_annotations = self.get_current_annotations(true);
            let current_script = self.env.current_filename.clone();
            let paths: Vec<String> = all_annotations
                .iter()
                .filter_map(|a| match a {
                    Annotation::SourcePath(x) => Some(x.clone()),
                    _ => None,
                })
                .collect();
            let external_sources = all_annotations.iter().find_map(|a| match a {
                Annotation::ExternalSources(b) => Some(*b),
                _ => None,
            });
            let resolved = sys.find_source(&current_script, external_sources, &paths, &filename);
            let contents = sys.read_file(external_sources, &resolved);
            (contents, resolved)
        };
        match input {
            Err(err) => {
                self.parse_note_at_id(file_id, InfoC, 1091, &format!("Not following: {err}"));
                Ok(t)
            }
            Ok(script) => {
                let id1 = self.get_new_id_for(cmd_id);
                let id2 = self.get_new_id_for(cmd_id);

                self.alt(
                    |p| {
                        let src = p.sub_read(&resolved_file, &script)?;
                        p.node(id2, Token::T_Include(src));
                        Ok(p.node(id1, Token::T_SourceCommand(t, id2)))
                    },
                    |p| {
                        p.parse_note_at_id(file_id, WarningC, 1094, "Parsing of sourced file failed. Ignoring it.");
                        Ok(t)
                    },
                )
            }
        }
    }

    /// `readSource`'s `subRead`
    fn sub_read(&mut self, name: &str, script: &str) -> PR<Id> {
        self.with_context(Context::ContextSource(name.to_string()), |p| {
            p.in_separate_context(|p| {
                let old_pending = std::mem::take(&mut p.user.pending_here_docs);
                let pos = p.initial_pos(name);
                let result = p.sub_parse(pos, |p| p.read_script_file(true), script)?;
                p.user.pending_here_docs = old_pending;
                Ok(result)
            })
        })
    }

    pub(crate) fn read_pipeline(&mut self) -> PR<Id> {
        self.unexpecting("keyword/token", |p| p.read_keyword())?;
        self.read_banged(&mut |p: &mut Self| p.read_pipe_sequence())
    }

    /// `parser` is a `dyn` reference because `readBanged` recurses with it.
    pub(crate) fn read_banged(&mut self, parser: &mut dyn FnMut(&mut Self) -> PR<Id>) -> PR<Id> {
        let mut parser = parser;
        self.alt_with(
            &mut parser,
            |p, parser| {
                let _pos = p.get_position();
                // `(T_Bang id) <- g_Bang`; `T_Banged id` reuses that id.
                let id = p.g_bang()?;
                let next = p.read_banged(&mut **parser)?;
                Ok(p.node(id, Token::T_Banged(next)))
            },
            |p, parser| (*parser)(p),
        )
    }

    pub(crate) fn read_and_or(&mut self) -> PR<Id> {
        let start = self.start_span();
        let apos = self.get_position();
        let annotations = self.read_annotations()?;
        let aid = self.end_span(start);

        if !annotations.is_empty() {
            self.optional(|p| {
                p.try_(|p| p.look_ahead(|p| p.read_keyword()))?;
                p.parse_problem_at(
                    apos,
                    ErrorC,
                    1123,
                    "ShellCheck directives are only valid in front of complete compound commands, like 'if', not e.g. individual 'elif' branches.",
                );
                Ok(())
            })?;
        }

        let and_or = self.with_annotations(annotations.clone(), |p| {
            p.chainl1(
                |p| p.read_pipeline(),
                |p| {
                    let op = p.alt(|p| p.g_and_if().map(|id| (true, id)), |p| p.g_or_if().map(|id| (false, id)))?;
                    p.read_line_break()?;
                    Ok(op)
                },
                // `T_AND_IF id -> T_AndIf id`: the operator's id is reused.
                |p, (is_and, id), x, y| {
                    if is_and { p.node(id, Token::T_AndIf(x, y)) } else { p.node(id, Token::T_OrIf(x, y)) }
                },
            )
        })?;

        if annotations.is_empty() { Ok(and_or) } else { Ok(self.node(aid, Token::T_Annotation(annotations, and_or))) }
    }

    #[allow(dead_code)] // unused in Parser.hs too
    pub(crate) fn read_term_or_none(&mut self) -> PR<Vec<Id>> {
        self.allspacing()?;
        self.alt(
            |p| p.read_term(),
            |p| {
                p.eof()?;
                Ok(Vec::new())
            },
        )
    }

    pub(crate) fn read_term(&mut self) -> PR<Vec<Id>> {
        self.allspacing()?;
        let m = self.read_and_or()?;
        self.read_term_(m)
    }

    /// `readTerm'`
    // DIVERGENCE: `readTerm'` recurses once per command; it is unrolled into
    // a loop. Once `readSeparator` has consumed, the recursive call is the
    // next step in sequence (as in `many_till`), so consumption and errors
    // are the same. Each separator's token is stored as soon as it is read
    // rather than after the recursion returns; a later failure fails the
    // whole term either way.
    fn read_term_(&mut self, current: Id) -> PR<Vec<Id>> {
        let mut acc = Vec::new();
        let mut current = current;
        loop {
            let step = self.alt(
                |p| {
                    let (sep, (start, end)) = p.read_separator()?;
                    let id = p.get_next_id_between(start, end);
                    // `option (T_EOF id) readAndOr`
                    let more = p.option_maybe(|p| p.read_and_or())?;
                    Ok(Some((sep, id, more)))
                },
                |_| Ok(None),
            )?;
            match step {
                None => {
                    acc.push(current);
                    return Ok(acc);
                }
                Some((sep, id, more)) => {
                    let t = self.transform_with_separator(id, sep, current);
                    acc.push(t);
                    match more {
                        None => return Ok(acc),
                        Some(m) => current = m,
                    }
                }
            }
        }
    }

    fn transform_with_separator(&mut self, i: Id, sep: char, current: Id) -> Id {
        if sep == '&' { self.node(i, Token::T_Backgrounded(current)) } else { current }
    }

    pub(crate) fn read_pipe_sequence(&mut self) -> PR<Id> {
        let start = self.start_span();
        // sepBy1WithSeparators p s = elems `chainl1` seps
        let (cmds, pipes) = self.chainl1(
            |p| {
                let x = p.read_banged(&mut |p: &mut Self| p.read_command())?;
                Ok((vec![x], Vec::new()))
            },
            |p| {
                p.then_skip(
                    |p| p.read_pipe(),
                    |p| {
                        p.spacing()?;
                        p.read_line_break()
                    },
                )
            },
            |_, separator, (mut a, mut b): (Vec<Id>, Vec<Id>), (c, d)| {
                a.extend(c);
                b.extend(d);
                b.push(separator);
                (a, b)
            },
        )?;
        let id = self.end_span(start);
        self.spacing()?;
        Ok(self.node(id, Token::T_Pipeline(pipes, cmds)))
    }

    pub(crate) fn read_pipe(&mut self) -> PR<Id> {
        self.not_followed_by2(|p| p.g_or_if())?;
        let start = self.start_span();
        self.char_('|')?;
        let qualifier = self.alt(|p| p.string("&"), |_| Ok(String::new()))?;
        let id = self.end_span(start);
        self.spacing()?;
        Ok(self.node(id, Token::T_Pipe(format!("|{qualifier}"))))
    }

    pub(crate) fn read_command(&mut self) -> PR<Id> {
        choice!(
            self,
            |p: &mut Self| p.read_compound_command(),
            |p: &mut Self| p.read_condition_command(),
            |p: &mut Self| p.read_co_proc(),
            |p: &mut Self| p.read_simple_command(),
        )
    }

    pub(crate) fn read_cmd_name(&mut self) -> PR<Id> {
        // If the command name is `!` then
        self.optional(|p| {
            p.look_ahead(|p| {
                p.try_(|p| {
                    p.char_('!')?;
                    p.whitespace()
                })
            })
        })?;
        // Ignore alias suppression
        self.optional(|p| {
            p.try_(|p| {
                p.char_('\\')?;
                p.look_ahead(|p| p.alt(|p| p.variable_chars(), |p| p.one_of(":.")))
            })
        })?;
        self.read_cmd_word()
    }

    pub(crate) fn read_cmd_word(&mut self) -> PR<Id> {
        self.skip_annotation_and_warn()?;
        let w = self.read_normal_word()?;
        self.spacing()?;
        Ok(w)
    }

    /// Due to poor planning, annotations after commands isn't handled well.
    /// At the time this function is used, it's usually too late to skip
    /// comments, so you end up with a parse failure instead.
    pub(crate) fn skip_annotation_and_warn(&mut self) -> PR<()> {
        self.optional(|p| {
            p.try_(|p| p.look_ahead(|p| p.read_annotation_prefix()))?;
            p.parse_problem(ErrorC, 1126, "Place shellcheck directives before commands, not after.");
            p.read_any_comment()
        })
    }
}

/// `isCommand strings cmd`, given `command_literal cmd`.
fn is_command(literal: &Option<String>, strings: &[&str]) -> bool {
    literal.as_deref().is_some_and(|s| strings.contains(&s))
}

/// `getParser def cmd table`
fn get_parser<'a>(def: SuffixReader<'a>, cmd: &Option<String>, table: &[(&[&str], SuffixReader<'a>)]) -> SuffixReader<'a> {
    match table {
        [] => def,
        [(list, action), rest @ ..] => {
            if is_command(cmd, list) {
                *action
            } else {
                get_parser(def, cmd, rest)
            }
        }
    }
}

/// `readSource`'s `getFile`
fn get_file(ast: &Ast, args: &[Id]) -> Option<Id> {
    let (first, rest) = args.split_first()?;
    match get_literal_string(ast, *first).as_deref() {
        Some("--") => rest.first().copied(),
        Some("-p") => rest.get(1).copied(),
        _ => Some(*first),
    }
}

/// If the word has a single expansion as the directory, try stripping it
/// This affects `$foo/bar` but not `${foo}-dir/bar` or `/foo/$file`
fn strip_dynamic_prefix(ast: &Ast, word: Id) -> Option<String> {
    match get_word_parts(ast, word).as_slice() {
        [exp, rest @ ..] if is_string_expansion(ast, *exp) => {
            // `getLiteralString (T_NormalWord (Id 0) rest)`
            let mut str = String::new();
            for &x in rest {
                str.push_str(&get_literal_string(ast, x)?);
            }
            if !str.starts_with('/') {
                return None;
            }
            Some(format!(".{str}"))
        }
        _ => None,
    }
}
