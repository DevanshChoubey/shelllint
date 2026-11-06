// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 2439-2908:
// if/while/until/for/select/case, functions, coproc, subshells, brace groups,
// bats tests, compound commands and lists.

use shellcheck_core::ast::{CaseType, FunctionKeyword, FunctionParentheses, Id, Token};
use shellcheck_core::interface::Severity::ErrorC;

use crate::engine::{P, PR, is_space};
use crate::state::IncompleteInterval;
use crate::{alt, choice};

impl<'a> P<'a> {
    pub(crate) fn read_if_clause(&mut self) -> PR<Id> {
        self.called("if expression", |p| {
            let start = p.start_span();
            let pos = p.get_position();
            let (condition, action) = p.read_if_part()?;
            let elifs = p.many(|p| p.read_elif_part())?;
            let elses = p.option(vec![], |p| p.read_else_part())?;

            p.or_fail(
                |p| p.g_fi(),
                |p| {
                    p.parse_problem_at(pos, ErrorC, 1046, "Couldn't find 'fi' for this 'if'.");
                    p.parse_problem(ErrorC, 1047, "Expected 'fi' matching previously mentioned 'if'.");
                    Ok("Expected 'fi'".to_string())
                },
            )?;
            let id = p.end_span(start);

            let mut parts = vec![(condition, action)];
            parts.extend(elifs);
            Ok(p.node(id, Token::T_IfExpression(parts, elses)))
        })
    }

    pub(crate) fn verify_not_empty_if(&mut self, s: &str) -> PR<()> {
        self.optional(|p| {
            let empty_pos = p.get_position();
            p.try_(|p| p.look_ahead(|p| alt!(p, |p| p.g_fi(), |p| p.g_elif(), |p| p.g_else())))?;
            p.parse_problem_at(
                empty_pos,
                ErrorC,
                1048,
                &format!("Can't have empty {s} clauses (use 'true' as a no-op)."),
            );
            Ok(())
        })
    }

    /// (condition, action)
    pub(crate) fn read_if_part(&mut self) -> PR<(Vec<Id>, Vec<Id>)> {
        let pos = self.get_position();
        self.g_if()?;
        self.allspacing()?;
        let condition = self.read_term()?;

        self.if_next_token(
            |p| alt!(p, |p| p.g_fi(), |p| p.g_elif(), |p| p.g_else()),
            |p| {
                p.parse_problem_at(pos, ErrorC, 1049, "Did you forget the 'then' for this 'if'?");
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

            p.accept_but_warn(
                |p| p.g_semi(),
                ErrorC,
                1051,
                "Semicolons directly after 'then' are not allowed. Just remove it.",
            )?;
            p.allspacing()?;
            p.verify_not_empty_if("then")?;

            let action = p.read_term()?;
            Ok((condition, action))
        })
    }

    /// (condition, action)
    pub(crate) fn read_elif_part(&mut self) -> PR<(Vec<Id>, Vec<Id>)> {
        self.called("elif clause", |p| {
            let pos = p.get_position();
            p.g_elif()?;
            p.allspacing()?;
            let condition = p.read_term()?;
            p.if_next_token(
                |p| alt!(p, |p| p.g_fi(), |p| p.g_elif(), |p| p.g_else()),
                |p| {
                    p.parse_problem_at(pos, ErrorC, 1049, "Did you forget the 'then' for this 'elif'?");
                    Ok(())
                },
            )?;

            p.g_then()?;
            p.accept_but_warn(
                |p| p.g_semi(),
                ErrorC,
                1052,
                "Semicolons directly after 'then' are not allowed. Just remove it.",
            )?;
            p.allspacing()?;
            p.verify_not_empty_if("then")?;
            let action = p.read_term()?;
            Ok((condition, action))
        })
    }

    pub(crate) fn read_else_part(&mut self) -> PR<Vec<Id>> {
        self.called("else clause", |p| {
            let pos = p.get_position();
            p.g_else()?;
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

            p.accept_but_warn(
                |p| p.g_semi(),
                ErrorC,
                1053,
                "Semicolons directly after 'else' are not allowed. Just remove it.",
            )?;
            p.allspacing()?;
            p.verify_not_empty_if("else")?;
            p.read_term()
        })
    }

    pub(crate) fn if_next_token<T>(
        &mut self,
        parser: impl FnOnce(&mut Self) -> PR<T>,
        action: impl FnOnce(&mut Self) -> PR<()>,
    ) -> PR<()> {
        self.optional(|p| {
            p.try_(|p| p.look_ahead(parser))?;
            action(p)
        })
    }

    pub(crate) fn read_subshell(&mut self) -> PR<Id> {
        self.called("explicit subshell", |p| {
            let start = p.start_span();
            p.char_('(')?;
            p.allspacing()?;
            let list = p.read_compound_list()?;
            p.allspacing()?;
            p.alt(|p| p.char_(')'), |p| p.fail("Expected ) closing the subshell"))?;
            let id = p.end_span(start);
            p.spacing()?;
            Ok(p.node(id, Token::T_Subshell(list)))
        })
    }

    pub(crate) fn read_brace_group(&mut self) -> PR<Id> {
        self.called("brace group", |p| {
            let start = p.start_span();
            p.char_('{')?;
            p.alt(
                |p| p.allspacing_or_fail().map(drop),
                |p| {
                    p.optional(|p| {
                        p.look_ahead(|p| p.none_of("("))?; // {( is legal
                        p.parse_problem(ErrorC, 1054, "You need a space after the '{'.");
                        Ok(())
                    })
                },
            )?;
            p.optional(|p| {
                let pos = p.get_position();
                p.look_ahead(|p| p.char_('}'))?;
                p.parse_problem_at(pos, ErrorC, 1055, "You need at least one command here. Use 'true;' as a no-op.");
                Ok(())
            })?;
            let list = p.read_term()?;
            p.alt(
                |p| p.char_('}'),
                |p| {
                    p.parse_problem(
                        ErrorC,
                        1056,
                        "Expected a '}'. If you have one, try a ; or \\n in front of it.",
                    );
                    p.fail("Missing '}'")
                },
            )?;
            let id = p.end_span(start);
            p.spacing()?;
            Ok(p.node(id, Token::T_BraceGroup(list)))
        })
    }

    pub(crate) fn read_bats_test(&mut self) -> PR<Id> {
        // We want everything before the last " {" in a string, so we find everything after "{ " in its reverse
        fn f(rev: &[char]) -> Vec<char> {
            let mut s = rev;
            loop {
                match s {
                    ['{', ' ', rest @ ..] => return rest.iter().copied().skip_while(|&c| is_space(c)).collect(),
                    [_, rest @ ..] => s = rest,
                    [] => return Vec::new(),
                }
            }
        }
        fn read_bats_name(p: &mut P<'_>) -> PR<String> {
            let line = p.try_(|p| p.look_ahead(|p| p.many1(|p| p.none_of("\n"))))?;
            let rev: Vec<char> = line.into_iter().rev().collect();
            let name: String = f(&rev).into_iter().rev().collect();
            p.string(&name)
        }

        self.called("bats @test", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("@test "))?;
            p.spacing()?;
            let name = read_bats_name(p)?;
            p.spacing()?;
            let test = p.read_brace_group()?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_BatsTest(name, test)))
        })
    }

    pub(crate) fn read_while_clause(&mut self) -> PR<Id> {
        self.called("while loop", |p| {
            let start = p.start_span();
            let kw_id = p.g_while()?;
            let condition = p.read_term()?;
            let statements = p.read_do_group(kw_id)?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_WhileExpression(condition, statements)))
        })
    }

    pub(crate) fn read_until_clause(&mut self) -> PR<Id> {
        self.called("until loop", |p| {
            let start = p.start_span();
            let kw_id = p.g_until()?;
            let condition = p.read_term()?;
            let statements = p.read_do_group(kw_id)?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_UntilExpression(condition, statements)))
        })
    }

    pub(crate) fn read_do_group(&mut self, kw_id: Id) -> PR<Vec<Id>> {
        self.optional(|p| {
            p.try_(|p| p.look_ahead(|p| p.g_done()))?;
            p.parse_problem_at_id(kw_id, ErrorC, 1057, "Did you forget the 'do' for this loop?");
            Ok(())
        })?;

        let do_kw = self.or_fail(
            |p| p.g_do(),
            |p| {
                p.parse_problem(ErrorC, 1058, "Expected 'do'.");
                Ok("Expected 'do'".to_string())
            },
        )?;

        self.accept_but_warn(
            |p| p.g_semi(),
            ErrorC,
            1059,
            "Semicolon is not allowed directly after 'do'. You can just delete it.",
        )?;
        self.allspacing()?;

        self.optional(|p| {
            p.try_(|p| p.look_ahead(|p| p.g_done()))?;
            p.parse_problem_at_id(do_kw, ErrorC, 1060, "Can't have empty do clauses (use 'true' as a no-op).");
            Ok(())
        })?;

        let commands = self.read_compound_list()?;
        self.or_fail(
            |p| p.g_done(),
            |p| {
                p.parse_problem_at_id(do_kw, ErrorC, 1061, "Couldn't find 'done' for this 'do'.");
                p.parse_problem(ErrorC, 1062, "Expected 'done' matching previously mentioned 'do'.");
                Ok("Expected 'done'".to_string())
            },
        )?;

        self.optional(|p| {
            p.look_ahead(|p| {
                let pos = p.get_position();
                p.try_(|p| p.string("<("))?;
                p.parse_problem_at(
                    pos,
                    ErrorC,
                    1142,
                    "Use 'done < <(cmd)' to redirect from process substitution (currently missing one '<').",
                );
                Ok(())
            })
        })?;
        Ok(commands)
    }

    pub(crate) fn read_for_clause(&mut self) -> PR<Id> {
        // For c='(' read "((" and be lenient about spaces
        fn read_arithmetic_delimiter(p: &mut P<'_>, c: char, msg: &str) -> PR<()> {
            p.char_(c)?;
            let start_pos = p.get_position();
            let sp = p.spacing()?;
            let end_pos = p.get_position();
            p.alt(
                |p| p.char_(c),
                |p| {
                    p.parse_problem_at(start_pos, ErrorC, 1137, msg);
                    p.fail("")
                },
            )?;
            if !sp.is_empty() {
                p.parse_problem_at_with_end(
                    start_pos,
                    end_pos,
                    ErrorC,
                    1138,
                    &format!("Remove spaces between {c}{c} in arithmetic for loop."),
                );
            }
            Ok(())
        }

        fn read_braced(p: &mut P<'_>) -> PR<Vec<Id>> {
            let group = p.read_brace_group()?;
            match p.user.ast.get(group) {
                Token::T_BraceGroup(list) => Ok(list.clone()),
                _ => unreachable!("readBraceGroup returns a T_BraceGroup"),
            }
        }

        // The loop token reuses the id of the `for` keyword, as in the
        // Haskell (`T_For id` -> `T_ForArithmetic id ...`), so the keyword's
        // slot is overwritten; this only happens on the success path.
        fn read_arithmetic(p: &mut P<'_>, id: Id) -> PR<Id> {
            p.called("arithmetic for condition", |p| {
                read_arithmetic_delimiter(p, '(', "Missing second '(' to start arithmetic for ((;;)) loop")?;
                let x = p.read_arithmetic_contents()?;
                p.char_(';')?;
                p.spacing()?;
                let y = p.read_arithmetic_contents()?;
                p.char_(';')?;
                p.spacing()?;
                let z = p.read_arithmetic_contents()?;
                p.spacing()?;
                read_arithmetic_delimiter(p, ')', "Missing second ')' to terminate 'for ((;;))' loop condition")?;
                p.spacing()?;
                p.optional(|p| {
                    p.read_sequential_sep()?;
                    p.spacing()
                })?;
                let group = p.alt(read_braced, |p| p.read_do_group(id))?;
                Ok(p.node(id, Token::T_ForArithmetic(x, y, z, group)))
            })
        }

        fn read_regular(p: &mut P<'_>, id: Id) -> PR<Id> {
            p.accept_but_warn(|p| p.char_('$'), ErrorC, 1086, "Don't use $ on the iterator name in for loops.")?;
            let name = p.then_skip(|p| p.read_variable_name(), |p| p.allspacing())?;
            let values = p.alt(
                |p| p.read_in_clause(),
                |p| {
                    p.optional(|p| p.read_sequential_sep())?;
                    Ok(vec![])
                },
            )?;
            let group = p.alt(read_braced, |p| p.read_do_group(id))?;
            Ok(p.node(id, Token::T_ForIn(name, values, group)))
        }

        self.called("for loop", |p| {
            let _pos = p.get_position();
            let id = p.g_for()?;
            p.spacing()?;
            p.alt(|p| read_arithmetic(p, id), |p| read_regular(p, id))
        })
    }

    pub(crate) fn read_select_clause(&mut self) -> PR<Id> {
        // DIVERGENCE: Haskell's `readRegular` returns a continuation
        // `\id group -> return $ T_SelectIn ...`; here it returns its parts.
        fn read_regular(p: &mut P<'_>) -> PR<(String, Vec<Id>)> {
            let name = p.read_variable_name()?;
            p.spacing()?;
            let values = p.alt(
                |p| p.read_in_clause(),
                |p| {
                    p.read_sequential_sep()?;
                    Ok(vec![])
                },
            )?;
            Ok((name, values))
        }

        self.called("select loop", |p| {
            let id = p.g_select()?;
            p.spacing()?;
            let (name, values) = read_regular(p)?;
            let group = p.read_do_group(id)?;
            Ok(p.node(id, Token::T_SelectIn(name, values, group)))
        })
    }

    pub(crate) fn read_in_clause(&mut self) -> PR<Vec<Id>> {
        self.g_in()?;
        let things = self.reluctantly_till(
            |p| p.read_cmd_word(),
            |p| alt!(p, |p| p.g_semi().map(drop), |p| p.linefeed().map(drop), |p| p.g_do().map(drop)),
        )?;

        self.alt(
            |p| {
                p.look_ahead(|p| p.g_do())?;
                p.parse_note(ErrorC, 1063, "You need a line feed or semicolon before the 'do'.");
                Ok(())
            },
            |p| {
                p.optional(|p| p.g_semi())?;
                p.allspacing().map(drop)
            },
        )?;

        Ok(things)
    }

    pub(crate) fn read_case_clause(&mut self) -> PR<Id> {
        self.called("case expression", |p| {
            let start = p.start_span();
            p.g_case()?;
            let word = p.read_normal_word()?;
            p.allspacing()?;
            p.alt(|p| p.g_in(), |p| p.fail("Expected 'in'"))?;
            p.read_line_break()?;
            let list = p.read_case_list()?;
            p.alt(|p| p.g_esac(), |p| p.fail("Expected 'esac' to close the case statement"))?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_CaseExpression(word, list)))
        })
    }

    pub(crate) fn read_case_list(&mut self) -> PR<Vec<(CaseType, Vec<Id>, Vec<Id>)>> {
        self.many(|p| p.read_case_item())
    }

    /// (separator, pattern, list)
    pub(crate) fn read_case_item(&mut self) -> PR<(CaseType, Vec<Id>, Vec<Id>)> {
        self.called("case item", |p| {
            p.not_followed_by2(|p| p.g_esac())?;
            p.optional(|p| {
                p.try_(|p| p.look_ahead(|p| p.read_annotation_prefix()))?;
                p.parse_problem(
                    ErrorC,
                    1124,
                    "ShellCheck directives are only valid in front of complete commands like 'case' statements, not individual case branches.",
                );
                Ok(())
            })?;
            p.optional(|p| p.g_lparen())?;
            p.spacing()?;
            let pattern = p.read_pattern()?;
            p.alt(
                |p| p.g_rparen().map(drop),
                |p| {
                    p.parse_problem(ErrorC, 1085, "Did you forget to move the ;; after extending this case item?");
                    p.fail("Expected ) to open a new case item")
                },
            )?;
            p.read_line_break()?;
            let list = p.alt(
                |p| {
                    p.look_ahead(|p| p.read_case_separator())?;
                    Ok(vec![])
                },
                |p| p.read_compound_list(),
            )?;
            let separator = p.attempting(
                |p| p.read_case_separator(),
                |p| {
                    let pos = p.get_position();
                    p.look_ahead(|p| p.g_rparen())?;
                    p.parse_problem_at(pos, ErrorC, 1074, "Did you forget the ;; after the previous case item?");
                    Ok(())
                },
            )?;
            p.read_line_break()?;
            Ok((separator, pattern, list))
        })
    }

    pub(crate) fn read_case_separator(&mut self) -> PR<CaseType> {
        choice!(
            self,
            |p| {
                p.try_token(";;&", |_, _| ())?;
                Ok(CaseType::CaseContinue)
            },
            |p| {
                p.try_token(";&", |_, _| ())?;
                Ok(CaseType::CaseFallThrough)
            },
            |p| {
                p.g_dsemi()?;
                Ok(CaseType::CaseBreak)
            },
            |p| {
                p.look_ahead(|p| {
                    p.read_line_break()?;
                    p.g_esac()
                })?;
                Ok(CaseType::CaseBreak)
            },
        )
    }

    pub(crate) fn read_function_definition(&mut self) -> PR<Id> {
        type Signature = (FunctionKeyword, FunctionParentheses, String);

        fn read_parens(p: &mut P<'_>) -> PR<()> {
            p.g_lparen()?;
            p.spacing()?;
            p.alt(
                |p| p.g_rparen(),
                |p| {
                    p.parse_problem(
                        ErrorC,
                        1065,
                        "Trying to declare parameters? Don't. Use () and refer to params as $1, $2..",
                    );
                    p.many(|p| p.none_of("\n){"))?;
                    p.g_rparen()
                },
            )?;
            Ok(())
        }

        fn read_with_function(p: &mut P<'_>) -> PR<Signature> {
            p.try_(|p| {
                p.string("function")?;
                p.whitespace()
            })?;
            p.spacing()?;
            let mut name = String::new();
            name.push(p.extended_function_start_chars()?);
            name.extend(p.many(|p| p.extended_function_chars())?);
            let spaces = p.spacing()?;
            let has_parens = p.was_included(read_parens)?;
            if !has_parens && spaces.is_empty() {
                p.accept_but_warn(
                    |p| p.look_ahead(|p| p.one_of("{(")),
                    ErrorC,
                    1095,
                    "You need a space or linefeed between the function name and body.",
                )?;
            }
            Ok((FunctionKeyword(true), FunctionParentheses(has_parens), name))
        }

        fn read_without_function(p: &mut P<'_>) -> PR<Signature> {
            p.try_(|p| {
                let mut name = String::new();
                name.push(p.function_start_chars()?);
                name.extend(p.many(|p| p.function_chars())?);
                p.guard(name != "time")?; // Interferes with time ( foo )
                p.spacing()?;
                read_parens(p)?;
                Ok((FunctionKeyword(false), FunctionParentheses(true), name))
            })
        }

        // DIVERGENCE: Haskell's signature parser returns a continuation
        // `\id -> T_Function id ...`; here it returns the fields.
        fn read_function_signature(p: &mut P<'_>) -> PR<Signature> {
            p.alt(read_with_function, read_without_function)
        }

        self.called("function", |p| {
            let start = p.start_span();
            let (kw, parens, name) = p.try_(read_function_signature)?;
            p.allspacing()?;
            p.alt(
                |p| p.look_ahead(|p| p.one_of("{(")).map(drop),
                |p| {
                    p.parse_problem(ErrorC, 1064, "Expected a { to open the function definition.");
                    Ok(())
                },
            )?;
            let group = p.alt(|p| p.read_brace_group(), |p| p.read_subshell())?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_Function(kw, parens, name, group)))
        })
    }

    pub(crate) fn read_co_proc(&mut self) -> PR<Id> {
        fn read_body(p: &mut P<'_>, parser: fn(&mut P<'_>) -> PR<Id>) -> PR<Id> {
            let start = p.start_span();
            let body = parser(p)?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_CoProcBody(body)))
        }

        fn read_compound_co_proc(p: &mut P<'_>, start: IncompleteInterval) -> PR<Id> {
            p.not_followed_by2(|p| p.read_assignment_word())?;
            let (var, body) = choice!(
                p,
                |p| p.try_(|p| {
                    let body = read_body(p, |p| p.read_compound_command())?;
                    Ok((None, body))
                }),
                |p| p.try_(|p| {
                    let var = p.then_skip(|p| p.read_normal_word(), |p| p.spacing())?;
                    let body = read_body(p, |p| p.read_compound_command())?;
                    Ok((Some(var), body))
                }),
            )?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_CoProc(var, body)))
        }

        fn read_simple_co_proc(p: &mut P<'_>, start: IncompleteInterval) -> PR<Id> {
            let body = read_body(p, |p| p.read_simple_command())?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_CoProc(None, body)))
        }

        self.called("coproc", |p| {
            let start = p.start_span();
            p.try_(|p| {
                p.string("coproc")?;
                p.spacing1()
            })?;
            choice!(p, |p| p.try_(|p| read_compound_co_proc(p, start)), |p| read_simple_co_proc(p, start))
        })
    }

    pub(crate) fn read_pattern(&mut self) -> PR<Vec<Id>> {
        self.sep_by1(
            |p| p.then_skip(|p| p.read_pattern_word(), |p| p.spacing()),
            |p| p.then_skip(|p| p.char_('|'), |p| p.spacing()),
        )
    }

    pub(crate) fn read_condition_command(&mut self) -> PR<Id> {
        fn alt_op(c: &str) -> &'static str {
            match c {
                "or" => "||",
                "-o" => "||",
                "and" => "&&",
                "-a" => "&&",
                _ => "|| or &&",
            }
        }

        let cmd = self.read_condition()?;
        let redirs = self.many(|p| p.read_io_redirect())?;
        let mut all = vec![cmd];
        all.extend(redirs.iter().copied());
        let id = self.get_next_id_spanning_token_list(&all);

        let pos = self.get_position();
        let has_dash_ao = self.is_followed_by(|p| {
            let c = p.choice(["-o", "-a", "or", "and"], |p, s| p.try_(|p| p.string(s)))?;
            let pos_end = p.get_position();
            p.parse_problem_at_with_end(
                pos,
                pos_end,
                ErrorC,
                1139,
                &format!("Use {} instead of '{}' between test commands.", alt_op(&c), c),
            );
            Ok(())
        })?;

        // If the next word is a keyword, readNormalWord will trigger a warning
        let has_keyword = self.is_followed_by(|p| p.read_keyword())?;
        let has_word = self.is_followed_by(|p| p.read_normal_word())?;

        if has_word && !(has_keyword || has_dash_ao) {
            // We have other words following, and no error has been emitted.
            let pos_end = self.get_position();
            self.parse_problem_at_with_end(
                pos,
                pos_end,
                ErrorC,
                1140,
                "Unexpected parameters after condition. Missing &&/||, or bad expression?",
            );
        }

        Ok(self.node(id, Token::T_Redirecting(redirs, cmd)))
    }

    pub(crate) fn read_compound_command(&mut self) -> PR<Id> {
        let cmd = choice!(
            self,
            |p| p.read_brace_group(),
            |p| p.read_ambiguous(
                "((",
                |p| p.read_arithmetic_expression(),
                |p| p.read_subshell(),
                |p, pos| {
                    p.parse_note_at(
                        pos,
                        ErrorC,
                        1105,
                        "Shells disambiguate (( differently or not at all. For subshell, add spaces around ( . For ((, fix parsing errors.",
                    );
                    Ok(())
                },
            ),
            |p| p.read_subshell(),
            |p| p.read_while_clause(),
            |p| p.read_until_clause(),
            |p| p.read_if_clause(),
            |p| p.read_for_clause(),
            |p| p.read_select_clause(),
            |p| p.read_case_clause(),
            |p| p.read_bats_test(),
            |p| p.read_function_definition(),
        )?;
        let redirs = self.many(|p| p.read_io_redirect())?;
        let mut all = vec![cmd];
        all.extend(redirs.iter().copied());
        let id = self.get_next_id_spanning_token_list(&all);
        self.optional(|p| {
            p.look_ahead(|p| {
                p.not_followed_by2(|p| choice!(p, |p| p.read_keyword(), |p| p.g_lbrace()))?;
                let pos = p.get_position();
                p.many1(|p| p.read_normal_word())?;
                let pos_end = p.get_position();
                p.parse_problem_at_with_end(
                    pos,
                    pos_end,
                    ErrorC,
                    1141,
                    "Unexpected tokens after compound command. Bad redirection or missing ;/&&/||/|?",
                );
                Ok(())
            })
        })?;
        Ok(self.node(id, Token::T_Redirecting(redirs, cmd)))
    }

    pub(crate) fn read_compound_list(&mut self) -> PR<Vec<Id>> {
        self.read_term()
    }

    pub(crate) fn read_compound_list_or_empty(&mut self) -> PR<Vec<Id>> {
        self.allspacing()?;
        self.alt(|p| p.read_term(), |_| Ok(vec![]))
    }
}
