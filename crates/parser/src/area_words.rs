// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1108-1611:
// words, quotes, backticks, globs, extglobs, escapes, brace expansions.

use shelllint_core::ast::{Id, Token};
use shelllint_core::astlib::only_literal_string;
use shelllint_core::interface::Severity::{ErrorC, InfoC, WarningC};

use crate::area_spacing::{QUOTABLE_CHARS, UNICODE_DOUBLE_QUOTES, UNICODE_SINGLE_QUOTES};
use crate::engine::{Input, P, PR, SourcePos, is_alpha};
use crate::{alt, choice};

// The char sets the Haskell readers build with `++` on every call, spelled
// out once (checked against the definitions in the tests below).
/// `"'\\" ++ unicodeSingleQuotes`
const SINGLE_QUOTED_PART_END: &str = "'\\\u{2018}\u{2019}";
/// `doubleQuotableChars ++ unicodeDoubleQuotes`
const DOUBLE_LITERAL_PART_END: &str = "\\\"$`\u{201C}\u{201D}\u{2033}\u{2036}";
/// `"![" ++ extglobStartChars`
const GLOB_CLASS_CHARS: &str = "![?*@!+";
/// `standardEnd = "[{}" ++ quotableChars ++ extglobStartChars ++ unicodeDoubleQuotes ++ unicodeSingleQuotes`
const NORMAL_LITERAL_STANDARD_END: &str =
    "[{}|&;<>()\\ '\t\n\r\u{A0}\\\"$`?*@!+\u{201C}\u{201D}\u{2033}\u{2036}\u{2018}\u{2019}";

impl<'a> P<'a> {
    pub(crate) fn read_normal_word(&mut self) -> PR<Id> {
        self.read_normalish_word("", &["do", "done", "then", "fi", "esac"])
    }

    pub(crate) fn read_pattern_word(&mut self) -> PR<Id> {
        self.read_normalish_word("", &["esac"])
    }

    pub(crate) fn read_normalish_word(&mut self, end: &str, terms: &[&str]) -> PR<Id> {
        let start = self.start_span();
        let pos = self.get_position();
        let x = self.many1(|p| p.read_normal_word_part(end))?;
        let id = self.end_span(start);
        self.check_possible_termination(pos, &x, terms);
        Ok(self.node(id, Token::T_NormalWord(x)))
    }

    pub(crate) fn read_index_span(&mut self) -> PR<Id> {
        let some_space = |p: &mut Self| {
            let start = p.start_span();
            let str = p.spacing1()?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Literal(str)))
        };
        let other_literal = |p: &mut Self| {
            let start = p.start_span();
            let str: String = p.many1(|p| p.one_of(QUOTABLE_CHARS))?.into_iter().collect();
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Literal(str)))
        };
        let start = self.start_span();
        let x = self.many(|p| alt!(p, |p: &mut Self| p.read_normal_word_part("]"), some_space, other_literal))?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_NormalWord(x)))
    }

    pub(crate) fn check_possible_termination(&mut self, pos: SourcePos, x: &[Id], terminators: &[&str]) {
        if let [t] = x
            && let Token::T_Literal(s) = self.user.ast.get(*t)
            && terminators.contains(&s.as_str())
        {
            let msg = format!("Use semicolon or linefeed before '{s}' (or quote to make it literal).");
            self.parse_problem_at(pos, WarningC, 1010, &msg);
        }
    }

    pub(crate) fn read_normal_word_part(&mut self, end: &str) -> PR<Id> {
        self.not_followed_by2(|p| p.one_of(end))?;
        // checkForParenthesis
        self.attempting(
            |_| Ok(()),
            |p| {
                let pos = p.get_position();
                p.look_ahead(|p| p.char_('('))?;
                p.parse_problem_at(pos, ErrorC, 1036, "'(' is invalid here. Did you forget to escape it?");
                Ok(())
            },
        )?;
        // readLiteralCurlyBraces
        let read_literal_curly_braces = |p: &mut Self| {
            let start = p.start_span();
            let str = p.alt(
                // findParam
                |p| p.try_(|p| p.string("{}")),
                // literalBraces
                |p| {
                    let pos = p.get_position();
                    let c = p.one_of("{}")?;
                    let msg = format!("This {c} is literal. Check expression (missing ;/\\n?) or quote it.");
                    p.parse_problem_at(pos, WarningC, 1083, &msg);
                    Ok(c.to_string())
                },
            )?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Literal(str)))
        };
        choice!(
            self,
            |p: &mut Self| p.read_single_quoted(),
            |p: &mut Self| p.read_double_quoted(),
            |p: &mut Self| p.read_glob(),
            |p: &mut Self| p.read_normal_dollar(),
            |p: &mut Self| p.read_braced(),
            |p: &mut Self| p.read_unquoted_back_ticked(),
            |p: &mut Self| p.read_proc_sub(),
            |p: &mut Self| p.read_unicode_quote(),
            |p: &mut Self| p.read_normal_literal(end),
            read_literal_curly_braces,
        )
    }

    pub(crate) fn read_space_part(&mut self) -> PR<Id> {
        let start = self.start_span();
        let x: String = self.many1(|p| p.whitespace())?.into_iter().collect();
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(x)))
    }

    pub(crate) fn read_dollar_braced_word(&mut self) -> PR<Id> {
        let start = self.start_span();
        let list = self.many(|p| p.read_dollar_braced_part())?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_NormalWord(list)))
    }

    pub(crate) fn read_dollar_braced_part(&mut self) -> PR<Id> {
        alt!(
            self,
            |p: &mut Self| p.read_single_quoted(),
            |p: &mut Self| p.read_double_quoted(),
            |p: &mut Self| p.read_param_sub_special_char(),
            |p: &mut Self| p.read_extglob(),
            |p: &mut Self| p.read_normal_dollar(),
            |p: &mut Self| p.read_unquoted_back_ticked(),
            |p: &mut Self| p.read_dollar_braced_literal(),
        )
    }

    pub(crate) fn read_dollar_braced_literal(&mut self) -> PR<Id> {
        let start = self.start_span();
        let vars = self.reluctantly_till1(
            |p| p.alt(|p| p.read_brace_escaped(), |p| p.any_char().map(String::from)),
            |p| p.braced_quotable(),
        )?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(vars.concat())))
    }

    pub(crate) fn read_param_sub_special_char(&mut self) -> PR<Id> {
        let start = self.start_span();
        let x: String = self.many1(|p| p.param_sub_special_chars())?.into_iter().collect();
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_ParamSubSpecialChar(x)))
    }

    pub(crate) fn read_proc_sub(&mut self) -> PR<Id> {
        self.called("process substitution", |p| {
            let start = p.start_span();
            let dir = p.try_(|p| {
                let x = p.one_of("<>")?;
                p.char_('(')?;
                Ok(x.to_string())
            })?;
            let list = p.read_compound_list_or_empty()?;
            p.allspacing()?;
            p.char_(')')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_ProcSub(dir, list)))
        })
    }

    pub(crate) fn read_single_quoted(&mut self) -> PR<Id> {
        self.called("single quoted string", |p| {
            let start = p.start_span();
            let start_pos = p.get_position();
            p.single_quote()?;
            let s = p.many(|p| p.read_single_quoted_part())?;
            let string = s.concat();
            let end_pos = p.get_position();
            p.alt(|p| p.single_quote(), |p| p.fail("Expected end of single quoted string"))?;

            p.optional(|p| {
                let c = p.try_(|p| p.look_ahead(|p| p.alt(|p| p.suspect_char_after_quotes(), |p| p.one_of("'"))))?;
                if !string.is_empty() && is_alpha(c) && string.chars().last().is_some_and(is_alpha) {
                    p.parse_problem_at(end_pos, WarningC, 1011, "This apostrophe terminated the single quoted string!");
                } else if string.contains('\n') && !string.starts_with('\n') {
                    p.suggest_forgot_closing_quote(start_pos, end_pos, "single quoted string");
                }
                Ok(())
            })?;

            let id = p.end_span(start);
            Ok(p.node(id, Token::T_SingleQuoted(string)))
        })
    }

    #[allow(dead_code)] // unused in Parser.hs too
    pub(crate) fn read_single_quoted_literal(&mut self) -> PR<String> {
        self.single_quote()?;
        let strs = self.many1(|p| p.read_single_quoted_part())?;
        self.single_quote()?;
        Ok(strs.concat())
    }

    pub(crate) fn read_single_quoted_part(&mut self) -> PR<String> {
        alt!(
            self,
            |p: &mut Self| p.read_single_escaped(),
            |p: &mut Self| p.many1(|p| p.none_of(SINGLE_QUOTED_PART_END)).map(|v| v.into_iter().collect()),
            // readUnicodeQuote
            |p: &mut Self| {
                let pos = p.get_position();
                let x = p.one_of(UNICODE_SINGLE_QUOTES)?;
                p.parse_problem_at(
                    pos,
                    WarningC,
                    1112,
                    "This is a unicode quote. Delete and retype it (or ignore/doublequote for literal).",
                );
                Ok(x.to_string())
            },
        )
    }

    pub(crate) fn read_quoted_back_ticked(&mut self) -> PR<Id> {
        self.read_back_ticked(true)
    }

    pub(crate) fn read_unquoted_back_ticked(&mut self) -> PR<Id> {
        self.read_back_ticked(false)
    }

    #[doc(hidden)] // pub for tests/gate.rs
    pub fn read_back_ticked(&mut self, quoted: bool) -> PR<Id> {
        fn un_escape(quoted: bool, s: &[char]) -> String {
            let mut out = String::new();
            let mut rest = s;
            loop {
                match rest {
                    [] => return out,
                    ['\\', '"', r @ ..] if quoted => {
                        out.push('"');
                        rest = r;
                    }
                    ['\\', x, r @ ..] if "$`\\".contains(*x) => {
                        out.push(*x);
                        rest = r;
                    }
                    ['\\', '\n', r @ ..] => rest = r,
                    [c, r @ ..] => {
                        out.push(*c);
                        rest = r;
                    }
                }
            }
        }
        let backtick = |p: &mut Self| {
            p.alt(
                |p| p.char_('`').map(drop),
                |p| {
                    let pos = p.get_position();
                    p.char_('´')?;
                    p.parse_problem_at(
                        pos,
                        ErrorC,
                        1077,
                        "For command expansion, the tick should slant left (` vs ´). Use $(..) instead.",
                    );
                    Ok(())
                },
            )
        };
        self.called("backtick expansion", |p| {
            let start = p.start_span();
            let start_pos = p.get_position();
            backtick(p)?;
            let sub_start = p.get_position();
            let sub_string = p.read_generic_literal("`´")?;
            let end_pos = p.get_position();
            backtick(p)?;
            let id = p.end_span(start);

            p.optional(|p| {
                let _c = p.try_(|p| p.look_ahead(|p| p.suspect_char_after_quotes()))?;
                if sub_string.contains('\n') && !sub_string.starts_with('\n') {
                    p.suggest_forgot_closing_quote(start_pos, end_pos, "backtick expansion");
                }
                Ok(())
            })?;

            // Result positions may be off due to escapes
            let chars: Vec<char> = sub_string.chars().collect();
            let input = un_escape(quoted, &chars);
            let result = p.sub_parse(
                sub_start,
                |p| {
                    p.alt(
                        |p| {
                            p.try_with_errors(|p| {
                                // subParser
                                let cmds = p.read_compound_list_or_empty()?;
                                p.verify_eof()?;
                                Ok(cmds)
                            })
                        },
                        |_| Ok(Vec::new()),
                    )
                },
                &input,
            )?;
            Ok(p.node(id, Token::T_Backticked(result)))
        })
    }

    /// Run a parser on a new input, such as for `..` or here documents.
    pub(crate) fn sub_parse<T>(
        &mut self,
        pos: SourcePos,
        parser: impl FnOnce(&mut Self) -> PR<T>,
        input: &str,
    ) -> PR<T> {
        let last_position = self.get_position();
        let last_input = self.get_input();
        self.set_position(pos);
        self.set_input(Input::new(input));
        let result = parser(self)?;
        self.set_input(last_input);
        self.set_position(last_position);
        Ok(result)
    }

    /// Parse something, but forget all parseProblems
    #[doc(hidden)] // pub for tests/gate.rs
    pub fn in_separate_context<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.parse_forgetting_context(true, parser)
    }

    /// Parse something, but forget all parseProblems on failure
    #[doc(hidden)] // pub for tests/gate.rs
    pub fn forget_on_failure<T>(&mut self, parser: impl FnOnce(&mut Self) -> PR<T>) -> PR<T> {
        self.parse_forgetting_context(false, parser)
    }

    pub(crate) fn parse_forgetting_context<T>(
        &mut self,
        also_on_success: bool,
        parser: impl FnOnce(&mut Self) -> PR<T>,
    ) -> PR<T> {
        let mut context = self.sys.save();
        self.alt_with(
            &mut context,
            // success
            |p, c| {
                let res = p.try_(parser)?;
                if also_on_success {
                    p.sys.restore(c);
                }
                Ok(res)
            },
            // failure
            |p, c| {
                p.sys.restore(c);
                p.fail("")
            },
        )
    }

    pub(crate) fn read_double_quoted(&mut self) -> PR<Id> {
        self.called("double quoted string", |p| {
            let start = p.start_span();
            let start_pos = p.get_position();
            p.double_quote()?;
            let x = p.many(|p| p.double_quoted_part())?;
            let end_pos = p.get_position();
            p.alt(|p| p.double_quote(), |p| p.fail("Expected end of double quoted string"))?;
            let id = p.end_span(start);
            p.optional(|p| {
                p.try_(|p| p.look_ahead(|p| p.alt(|p| p.suspect_char_after_quotes(), |p| p.one_of("$\""))))?;
                let has_line_feed = |t: &Id| matches!(p.user.ast.get(*t), Token::T_Literal(s) if s.contains('\n'));
                let starts_with_line_feed = match x.first() {
                    Some(t) => matches!(p.user.ast.get(*t), Token::T_Literal(s) if s.starts_with('\n')),
                    None => false,
                };
                if x.iter().any(has_line_feed) && !starts_with_line_feed {
                    p.suggest_forgot_closing_quote(start_pos, end_pos, "double quoted string");
                }
                Ok(())
            })?;
            Ok(p.node(id, Token::T_DoubleQuoted(x)))
        })
    }

    pub(crate) fn suggest_forgot_closing_quote(&mut self, start_pos: SourcePos, end_pos: SourcePos, name: &str) {
        self.parse_problem_at(start_pos, WarningC, 1078, &format!("Did you forget to close this {name}?"));
        self.parse_problem_at(
            end_pos,
            InfoC,
            1079,
            "This is actually an end quote, but due to next char it looks suspect.",
        );
    }

    pub(crate) fn double_quoted_part(&mut self) -> PR<Id> {
        alt!(
            self,
            |p: &mut Self| p.read_double_literal(),
            |p: &mut Self| p.read_double_quoted_dollar(),
            |p: &mut Self| p.read_quoted_back_ticked(),
            // readUnicodeQuote
            |p: &mut Self| {
                let pos = p.get_position();
                let start = p.start_span();
                let c = p.one_of(UNICODE_DOUBLE_QUOTES)?;
                let id = p.end_span(start);
                p.parse_problem_at(
                    pos,
                    WarningC,
                    1111,
                    "This is a unicode quote. Delete and retype it (or ignore/singlequote for literal).",
                );
                Ok(p.node(id, Token::T_Literal(c.to_string())))
            },
        )
    }

    pub(crate) fn read_double_literal(&mut self) -> PR<Id> {
        let start = self.start_span();
        let s = self.many1(|p| p.read_double_literal_part())?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(s.concat())))
    }

    pub(crate) fn read_double_literal_part(&mut self) -> PR<String> {
        let x = self.many1(|p| {
            p.alt(
                |p| p.read_double_escaped(),
                |p| p.many1(|p| p.none_of(DOUBLE_LITERAL_PART_END)).map(|v| v.into_iter().collect()),
            )
        })?;
        Ok(x.concat())
    }

    pub(crate) fn read_normal_literal(&mut self, end: &str) -> PR<Id> {
        let start = self.start_span();
        let s = self.many1(|p| p.read_normal_literal_part(end))?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(s.concat())))
    }

    pub(crate) fn read_glob(&mut self) -> PR<Id> {
        let read_simple = |p: &mut Self| {
            let start = p.start_span();
            let c = p.one_of("*?")?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Glob(c.to_string())))
        };
        let read_class = |p: &mut Self| {
            p.try_(|p| {
                let start = p.start_span();
                p.char_('[')?;
                let negation = p.alt(|p| p.one_of("!^").map(String::from), |_| Ok(String::new()))?;
                let leading_bracket = p.alt(|p| p.one_of("]").map(String::from), |_| Ok(String::new()))?;
                let s = p.many(|p| {
                    alt!(
                        p,
                        // predefined
                        |p: &mut Self| {
                            p.try_(|p| p.string("[:"))?;
                            let s: String = p.many1(|p| p.letter())?.into_iter().collect();
                            p.string(":]")?;
                            Ok(format!("[:{s}:]"))
                        },
                        |p: &mut Self| p.read_normal_literal_part("]"),
                        // globchars
                        |p: &mut Self| p.one_of(GLOB_CLASS_CHARS).map(String::from),
                    )
                })?;
                p.guard(!leading_bracket.is_empty() || !s.is_empty())?;
                p.char_(']')?;
                let id = p.end_span(start);
                let glob = format!("[{negation}{leading_bracket}{}]", s.concat());
                Ok(p.node(id, Token::T_Glob(glob)))
            })
        };
        let read_globby_literal = |p: &mut Self| {
            let start = p.start_span();
            let c = p.alt(|p| p.extglob_start(), |p| p.char_('['))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Literal(c.to_string())))
        };
        alt!(self, |p: &mut Self| p.read_extglob(), read_simple, read_class, read_globby_literal)
    }

    pub(crate) fn read_normal_literal_part(&mut self, custom_end: &str) -> PR<String> {
        // noneOf (customEnd ++ standardEnd)
        let not_these = |c: char| custom_end.contains(c) || NORMAL_LITERAL_STANDARD_END.contains(c);
        self.alt(
            |p| p.read_normal_escaped(),
            |p| p.many1(|p| p.satisfy(|c| !not_these(c))).map(|v| v.into_iter().collect()),
        )
    }

    pub(crate) fn read_normal_escaped(&mut self) -> PR<String> {
        fn alternative(c: char) -> String {
            match c {
                'n' => "a quoted, literal line feed".to_string(),
                t => format!("\"$(printf '\\{t}')\""),
            }
        }
        fn escaped_char(c: char) -> Option<&'static str> {
            match c {
                'n' => Some("line feed"),
                't' => Some("tab"),
                'r' => Some("carriage return"),
                _ => None,
            }
        }
        let check_trailing_spaces = |p: &mut Self, pos: SourcePos| {
            p.look_ahead(|p| {
                p.try_(|p| {
                    p.many(|p| p.linewhitespace())?;
                    p.alt(|p| p.linefeed().map(drop), |p| p.eof())?;
                    p.parse_problem_at(
                        pos,
                        ErrorC,
                        1101,
                        "Delete trailing spaces after \\ to break line (or use quotes for literal space).",
                    );
                    Ok(())
                })
            })
        };
        self.called("escaped char", |p| {
            let pos = p.get_position();
            p.backslash()?;
            p.alt(
                |p| {
                    let next = p.alt(|p| p.quotable(), |p| p.one_of("?*@!+[]{}.,~#"))?;
                    if next == ' ' {
                        p.alt(|p| check_trailing_spaces(p, pos), |_| Ok(()))?;
                    }
                    // Check if this line is followed by a commented line with a trailing backslash
                    if next == '\n' {
                        p.try_(|p| p.look_ahead(|p| p.spacing().map(drop)))?;
                    }
                    Ok(if next == '\n' { String::new() } else { next.to_string() })
                },
                |p| {
                    let next = p.any_char()?;
                    match escaped_char(next) {
                        Some(name) => {
                            let msg = format!(
                                "\\{next} is just literal '{next}' here. For {name}, use {} instead.",
                                alternative(next)
                            );
                            p.parse_note_at(pos, WarningC, 1012, &msg);
                        }
                        None => {
                            let msg = format!("This \\{next} will be a regular '{next}' in this context.");
                            p.parse_note_at(pos, InfoC, 1001, &msg);
                        }
                    }
                    Ok(next.to_string())
                },
            )
        })
    }

    pub(crate) fn read_extglob(&mut self) -> PR<Id> {
        self.called("extglob", |p| {
            let start = p.start_span();
            let c = p.try_(|p| {
                let f = p.extglob_start()?;
                p.char_('(')?;
                Ok(f)
            })?;
            let contents = p.sep_by(|p| p.read_extglob_part(), |p| p.char_('|'))?;
            p.char_(')')?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Extglob(c.to_string(), contents)))
        })
    }

    pub(crate) fn read_extglob_part(&mut self) -> PR<Id> {
        let read_extglob_group = |p: &mut Self| {
            p.char_('(')?;
            let start = p.start_span();
            let contents = p.sep_by(|p| p.read_extglob_part(), |p| p.char_('|'))?;
            let id = p.end_span(start);
            p.char_(')')?;
            Ok(p.node(id, Token::T_Extglob(String::new(), contents)))
        };
        let read_extglob_literal = |p: &mut Self| {
            let start = p.start_span();
            let str: String = p.many1(|p| p.one_of("<>#;&"))?.into_iter().collect();
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Literal(str)))
        };
        let start = self.start_span();
        let x = self.many(|p| {
            alt!(
                p,
                read_extglob_group,
                |p: &mut Self| p.read_normal_word_part(""),
                |p: &mut Self| p.read_space_part(),
                read_extglob_literal,
            )
        })?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_NormalWord(x)))
    }

    pub(crate) fn read_single_escaped(&mut self) -> PR<String> {
        let pos = self.get_position();
        let s = self.backslash()?;
        let x = self.look_ahead(|p| p.any_char())?;

        if x == '\'' {
            self.parse_problem_at(pos, InfoC, 1003, "Want to escape a single quote? echo 'This is how it'\\''s done'.");
        }

        Ok(s.to_string())
    }

    pub(crate) fn read_double_escaped(&mut self) -> PR<String> {
        let _pos = self.get_position();
        let bs = self.backslash()?;
        alt!(
            self,
            |p: &mut Self| p.linefeed().map(|_| String::new()),
            |p: &mut Self| p.double_quotable().map(String::from),
            |p: &mut Self| {
                let c = p.any_char()?;
                // This is an invalid escape sequence where the \ is literal.
                // Previously this caused a SC1117, which may be re-enabled as
                // as a pedantic warning.
                Ok([bs, c].iter().collect())
            },
        )
    }

    pub(crate) fn read_brace_escaped(&mut self) -> PR<String> {
        let bs = self.backslash()?;
        alt!(
            self,
            |p: &mut Self| p.linefeed().map(|_| String::new()),
            |p: &mut Self| p.braced_quotable().map(String::from),
            |p: &mut Self| p.any_char().map(|x| [bs, x].iter().collect()),
        )
    }

    pub(crate) fn read_generic_literal(&mut self, end_chars: &str) -> PR<String> {
        // noneOf ('\\':endChars)
        let not_these = |c: char| c == '\\' || end_chars.contains(c);
        let strings = self.many(|p| {
            p.alt(
                |p| p.read_generic_escaped(),
                |p| p.many1(|p| p.satisfy(|c| !not_these(c))).map(|v| v.into_iter().collect()),
            )
        })?;
        Ok(strings.concat())
    }

    pub(crate) fn read_generic_literal1<T>(&mut self, end_exp: impl FnMut(&mut Self) -> PR<T>) -> PR<String> {
        let strings = self
            .reluctantly_till1(|p| p.alt(|p| p.read_generic_escaped(), |p| p.any_char().map(String::from)), end_exp)?;
        Ok(strings.concat())
    }

    pub(crate) fn read_generic_escaped(&mut self) -> PR<String> {
        self.backslash()?;
        let x = self.any_char()?;
        Ok(if x == '\n' { String::new() } else { ['\\', x].iter().collect() })
    }

    pub(crate) fn read_braced(&mut self) -> PR<Id> {
        self.try_(|p| p.brace_expansion())
    }

    /// `readBraced`'s `braceExpansion`.
    fn brace_expansion(&mut self) -> PR<Id> {
        self.with_parser(
            |p, id, elements| p.node(id, Token::T_BraceExpansion(elements)),
            |p| {
                p.char_('{')?;
                let elements = p.sep_by1(|p| p.braced_element(), |p| p.char_(','))?;
                let ok = match elements.as_slice() {
                    [_, _, ..] => true,
                    [t] => only_literal_string(&p.user.ast, *t).contains(".."),
                    [] => false,
                };
                p.guard(ok)?;
                p.char_('}')?;
                Ok(elements)
            },
        )
    }

    /// `readBraced`'s `bracedElement`.
    fn braced_element(&mut self) -> PR<Id> {
        self.with_parser(
            |p, id, parts| p.node(id, Token::T_NormalWord(parts)),
            |p| {
                p.many(|p| {
                    choice!(
                        p,
                        |p: &mut Self| p.brace_expansion(),
                        |p: &mut Self| p.read_dollar_expression(),
                        |p: &mut Self| p.read_single_quoted(),
                        |p: &mut Self| p.read_double_quoted(),
                        // braceLiteral
                        |p: &mut Self| {
                            p.with_parser(
                                |p, id, s| p.node(id, Token::T_Literal(s)),
                                |p| p.read_generic_literal1(|p| p.alt(|p| p.one_of("{}\"$',"), |p| p.whitespace())),
                            )
                        },
                    )
                })
            },
        )
    }
}

#[cfg(test)]
mod char_set_tests {
    use super::*;
    use crate::area_spacing::{DOUBLE_QUOTABLE_CHARS, EXTGLOB_START_CHARS};

    #[test]
    fn spelled_out_char_sets_match_their_definitions() {
        assert_eq!(SINGLE_QUOTED_PART_END, format!("'\\{UNICODE_SINGLE_QUOTES}"));
        assert_eq!(DOUBLE_LITERAL_PART_END, format!("{DOUBLE_QUOTABLE_CHARS}{UNICODE_DOUBLE_QUOTES}"));
        assert_eq!(GLOB_CLASS_CHARS, format!("![{EXTGLOB_START_CHARS}"));
        assert_eq!(
            NORMAL_LITERAL_STANDARD_END,
            format!("[{{}}{QUOTABLE_CHARS}{EXTGLOB_START_CHARS}{UNICODE_DOUBLE_QUOTES}{UNICODE_SINGLE_QUOTES}")
        );
    }
}
