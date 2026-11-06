// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 464-712:
// readConditionContents, the contents of [ ] and [[ ]].
//
// The `where` clause of readConditionContents becomes the `cond_*` methods
// below, each taking `single` (the Haskell closure variable) explicitly.

use shellcheck_core::ast::{Ast, ConditionType, Id, Token};
use shellcheck_core::astlib::only_literal_string;
use shellcheck_core::data::{BINARY_TEST_OPS, COMMON_COMMANDS};
use shellcheck_core::interface::Severity::ErrorC;
use shellcheck_core::interface::Severity::WarningC;

use crate::alt;
use crate::engine::{P, PR};

type PFn<'a, T> = fn(&mut P<'a>) -> PR<T>;

const WEIRD_DASHES: &str = "\u{058A}\u{05BE}\u{2010}\u{2011}\u{2012}\u{2013}\u{2014}\u{2015}\u{FE63}\u{FF0D}";

impl<'a> P<'a> {
    pub(crate) fn read_condition_contents(&mut self, single: bool) -> PR<Id> {
        self.attempting(
            |p| p.cond_contents(single),
            |p| {
                p.look_ahead(|p| {
                    let pos = p.get_position();
                    let s = p.read_variable_name()?;
                    p.spacing1()?;
                    if COMMON_COMMANDS.contains(&s.as_str()) {
                        p.parse_problem_at(
                            pos,
                            WarningC,
                            1014,
                            "Use 'if cmd; then ..' to check exit code, or 'if [[ $(cmd) == .. ]]' to check output.",
                        );
                    }
                    Ok(())
                })
            },
        )
    }

    fn cond_spacing_or_lf(&mut self, single: bool) -> PR<String> {
        self.cond_spacing(single, true)
    }

    fn cond_spacing(&mut self, single: bool, required: bool) -> PR<String> {
        let pos = self.get_position();
        let space = self.allspacing()?;
        if required && space.is_empty() {
            self.parse_problem_at(pos, ErrorC, 1035, "You are missing a required space here.");
        }
        if single && space.contains('\n') {
            self.parse_problem_at(pos, ErrorC, 1080, "When breaking lines in [ ], you need \\ before the linefeed.");
        }
        Ok(space)
    }

    fn cond_typ(single: bool) -> ConditionType {
        if single { ConditionType::SingleBracket } else { ConditionType::DoubleBracket }
    }

    /// Returns the id and operator of the (not yet built) `TC_Binary`.
    fn cond_read_binary_op(&mut self, single: bool) -> PR<(Id, String)> {
        self.try_(|p| {
            p.optional(|p| p.cond_guard_arithmetic(single))?;
            let op = p.cond_get_op()?;
            p.cond_spacing_or_lf(single)?;
            Ok(op)
        })
    }

    fn cond_get_op(&mut self) -> PR<(Id, String)> {
        let start = self.start_span();
        let op = self.cond_read_regular_or_escaped(|p| p.cond_any_op())?;
        let id = self.end_span(start);
        Ok((id, op))
    }

    fn cond_any_op(&mut self) -> PR<String> {
        alt!(
            self,
            |p| p.cond_flag_op(),
            |p| p.cond_flagless_op(),
            |p| p.fail("Expected comparison operator (don't wrap commands in []/[[]])")
        )
    }

    fn cond_flag_op(&mut self) -> PR<String> {
        self.try_(|p| {
            let s = p.cond_read_op()?;
            if s == "-a" || s == "-o" {
                return p.fail("Unexpected operator");
            }
            Ok(s)
        })
    }

    fn cond_flagless_op(&mut self) -> PR<String> {
        let flagless_ops = ["==", "!=", "<=", ">=", "=~", ">", "<", "="];
        self.choice(flagless_ops, |p, s| p.try_(|p| p.string(s)))
    }

    // hacks to read quoted operators without having to read a shell word
    fn cond_read_escaped(&mut self, p: PFn<'a, String>) -> PR<String> {
        fn escaped(s: String) -> String {
            if s.chars().any(|c| "<>()".contains(c)) { format!("\\{s}") } else { s }
        }
        self.try_(|q| {
            q.alt(
                |q| {
                    // withEscape
                    q.char_('\\')?;
                    Ok(escaped(p(q)?))
                },
                |q| {
                    // withQuotes
                    let c = q.one_of("'\"")?;
                    let s = p(q)?;
                    q.char_(c)?;
                    Ok(escaped(s))
                },
            )
        })
    }

    fn cond_read_regular_or_escaped(&mut self, p: PFn<'a, String>) -> PR<String> {
        self.alt(|q| q.cond_read_escaped(p), |q| p(q))
    }

    fn cond_guard_arithmetic(&mut self, single: bool) -> PR<()> {
        self.try_(|p| {
            p.look_ahead(|p| {
                p.alt(
                    |p| p.one_of("+*/%").map(drop),
                    |p| p.string("- ").map(drop),
                )
            })
        })?;
        self.parse_problem(
            ErrorC,
            1076,
            if single {
                "Trying to do math? Use e.g. [ $((i/2+7)) -ge 18 ]."
            } else {
                "Trying to do math? Use e.g. [[ $((i/2+7)) -ge 18 ]]."
            },
        );
        Ok(())
    }

    fn cond_read_unary_exp(&mut self, single: bool) -> PR<Id> {
        let (id, s) = self.cond_read_unary_op(single)?;
        let pos = self.get_position();
        self.or_fail(
            |p| {
                let x = p.cond_read_word(single)?;
                Ok(p.node(id, Token::TC_Unary(Self::cond_typ(single), s, x)))
            },
            |p| {
                p.parse_problem_at(pos, ErrorC, 1019, "Expected this to be an argument to the unary condition.");
                Ok("Expected an argument for the unary operator".to_string())
            },
        )
    }

    fn cond_read_unary_op(&mut self, single: bool) -> PR<(Id, String)> {
        self.try_(|p| {
            let start = p.start_span();
            let s = p.cond_read_op()?;
            let id = p.end_span(start);
            p.cond_spacing_or_lf(single)?;
            Ok((id, s))
        })
    }

    fn cond_read_op(&mut self) -> PR<String> {
        self.try_(|p| {
            p.alt(|p| p.char_('-'), |p| p.cond_weird_dash())?;
            let s = p.alt(|p| p.many1(|p| p.letter()), |p| p.fail("Expected a test operator"))?;
            Ok(format!("-{}", s.into_iter().collect::<String>()))
        })
    }

    fn cond_weird_dash(&mut self) -> PR<char> {
        let pos = self.get_position();
        self.one_of(WEIRD_DASHES)?;
        self.parse_problem_at(pos, ErrorC, 1100, "This is a unicode dash. Delete and retype as ASCII minus.");
        Ok('-')
    }

    fn cond_read_word(&mut self, single: bool) -> PR<Id> {
        self.not_followed_by2(|p| {
            p.try_(|p| {
                p.spacing()?;
                p.string("]")
            })
        })?;
        let x = self.read_normal_word()?;
        let pos = self.get_position();
        if cond_not_array_index(&self.user.ast, x)
            && cond_ended_with(&self.user.ast, "]", x)
            && !only_literal_string(&self.user.ast, x).contains('[')
        {
            self.parse_problem_at(
                pos,
                ErrorC,
                1020,
                &format!("You need a space before the {}.", if single { "]" } else { "]]" }),
            );
            return self.fail("Missing space before ]");
        }
        if single && cond_ended_with(&self.user.ast, ")", x) {
            self.parse_problem_at(pos, ErrorC, 1021, "You need a space before the \\)");
            return self.fail("Missing space before )");
        }
        self.spacing()?;
        Ok(x)
    }

    fn cond_read_and_op(&mut self, single: bool) -> PR<(Id, String)> {
        self.alt(
            |p| p.cond_read_and_or_op(single, "&&", false),
            |p| p.cond_read_and_or_op(single, "-a", true),
        )
    }

    fn cond_read_or_op(&mut self, single: bool) -> PR<(Id, String)> {
        self.optional(|p| p.cond_guard_arithmetic(single))?;
        self.alt(
            |p| p.cond_read_and_or_op(single, "||", false),
            |p| p.cond_read_and_or_op(single, "-o", true),
        )
    }

    /// Returns the id and operator of the (not yet built) `TC_And`/`TC_Or`.
    fn cond_read_and_or_op(&mut self, single: bool, op: &str, requires_spacing: bool) -> PR<(Id, String)> {
        self.optional(|p| p.look_ahead(|p| p.cond_weird_dash()))?;
        let start = self.start_span();
        let x = self.try_(|p| p.string(op))?;
        let id = self.end_span(start);
        self.cond_spacing(single, requires_spacing)?;
        Ok((id, x))
    }

    fn cond_read_nullary_or_binary(&mut self, single: bool) -> PR<Id> {
        let start = self.start_span();
        let x = self.attempting(
            |p| p.cond_read_word(single),
            |p| {
                let pos = p.get_position();
                p.look_ahead(|p| p.char_('['))?;
                p.parse_problem_at(
                    pos,
                    ErrorC,
                    1026,
                    if single {
                        "If grouping expressions inside [..], use \\( ..\\)."
                    } else {
                        "If grouping expressions inside [[..]], use ( .. )."
                    },
                );
                Ok(())
            },
        )?;
        let id = self.end_span(start);
        self.alt(
            |p| {
                let pos = p.get_position();
                let is_regex = p.cond_regex_operator_ahead()?;
                let (op_id, op) = p.cond_read_binary_op(single)?;
                let y = if is_regex {
                    p.cond_read_regex()?
                } else {
                    p.alt(
                        |p| p.cond_read_word(single),
                        |p| {
                            p.parse_problem_at(pos, ErrorC, 1027, "Expected another argument for this operator.");
                            p.zero()
                        },
                    )?
                };
                Ok(p.node(op_id, Token::TC_Binary(Self::cond_typ(single), op, x, y)))
            },
            |p| {
                p.cond_check_trailing_op(x);
                Ok(p.node(id, Token::TC_Nullary(Self::cond_typ(single), x)))
            },
        )
    }

    fn cond_check_trailing_op(&mut self, x: Id) {
        let ast = &self.user.ast;
        let found = match ast.get(x) {
            Token::T_NormalWord(list) => list.last().copied().and_then(|last| match ast.get(last) {
                Token::T_Literal(str) => BINARY_TEST_OPS
                    .iter()
                    .find(|op| str.ends_with(**op))
                    .map(|op| (last, *op)),
                _ => None,
            }),
            _ => None,
        };
        if let Some((id, trailing_op)) = found {
            self.parse_problem_at_id(
                id,
                ErrorC,
                1108,
                &format!("You need a space before and after the {trailing_op} ."),
            );
        }
    }

    fn cond_read_group(&mut self, single: bool) -> PR<Id> {
        let start = self.start_span();
        let pos = self.get_position();
        let lparen = self.try_(|p| p.cond_read_regular_or_escaped(|p| p.string("(")))?;
        if single && lparen == "(" {
            Self::cond_single_warning(self, pos);
        }
        if !single && lparen == "\\(" {
            Self::cond_double_warning(self, pos);
        }
        self.cond_spacing(single, single)?;
        let x = self.cond_contents(single)?;
        let cpos = self.get_position();
        let rparen = self.cond_read_regular_or_escaped(|p| p.string(")"))?;
        let id = self.end_span(start);
        self.cond_spacing(single, single)?;
        if single && rparen == ")" {
            Self::cond_single_warning(self, cpos);
        }
        if !single && rparen == "\\)" {
            Self::cond_double_warning(self, cpos);
        }
        Ok(self.node(id, Token::TC_Group(Self::cond_typ(single), x)))
    }

    fn cond_single_warning(&mut self, pos: crate::engine::SourcePos) {
        self.parse_problem_at(
            pos,
            ErrorC,
            1028,
            "In [..] you have to escape \\( \\) or preferably combine [..] expressions.",
        );
    }

    fn cond_double_warning(&mut self, pos: crate::engine::SourcePos) {
        self.parse_problem_at(pos, ErrorC, 1029, "In [[..]] you shouldn't escape ( or ).");
    }

    // Currently a bit of a hack since parsing rules are obscure
    fn cond_regex_operator_ahead(&mut self) -> PR<bool> {
        self.alt(
            |p| {
                p.look_ahead(|p| {
                    p.alt(|p| p.try_(|p| p.string("=~")), |p| p.try_(|p| p.string("~=")))?;
                    Ok(true)
                })
            },
            |_| Ok(false),
        )
    }

    fn cond_read_regex(&mut self) -> PR<Id> {
        self.called("regex", |p| {
            let start = p.start_span();
            let parts = p.many1(|p| p.cond_regex_part())?;
            let id = p.end_span(start);
            p.spacing()?;
            Ok(p.node(id, Token::T_NormalWord(parts)))
        })
    }

    fn cond_regex_part(&mut self) -> PR<Id> {
        crate::choice!(
            self,
            |p| p.cond_regex_group(),
            |p| p.read_single_quoted(),
            |p| p.read_double_quoted(),
            |p| p.read_dollar_expression(),
            |p| p.read_literal_for_parser(|p| p.read_normal_literal("( ")),
            |p| p.cond_regex_literal_string("|"),
            |p| p.cond_regex_glob_literal()
        )
    }

    fn cond_regex_glob_literal(&mut self) -> PR<Id> {
        let start = self.start_span();
        let s = self.alt(|p| p.extglob_start(), |p| p.one_of("{}[]$"))?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(s.to_string())))
    }

    fn cond_regex_group(&mut self) -> PR<Id> {
        self.called("regex grouping", |p| {
            let start = p.start_span();
            let p1 = p.cond_regex_literal_string("(")?;
            let mut parts = p.many(|p| p.alt(|p| p.cond_regex_part(), |p| p.cond_regex_literal()))?;
            let p2 = p.cond_regex_literal_string(")")?;
            let id = p.end_span(start);
            parts.insert(0, p1);
            parts.push(p2);
            Ok(p.node(id, Token::T_NormalWord(parts)))
        })
    }

    fn cond_regex_literal(&mut self) -> PR<Id> {
        let start = self.start_span();
        let str = self.read_generic_literal1(|p| {
            alt!(p, |p| p.single_quote(), |p| p.double_quotable(), |p| p.one_of("()"))
        })?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(str)))
    }

    fn cond_regex_literal_string(&mut self, s: &str) -> PR<Id> {
        let start = self.start_span();
        let str = self.string(s)?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(str)))
    }

    fn cond_read_term(&mut self, single: bool) -> PR<Id> {
        let term = self.alt(|p| p.cond_read_not(single), |p| p.cond_read_expr(single))?;
        self.cond_spacing(single, false)?;
        Ok(term)
    }

    fn cond_read_not(&mut self, single: bool) -> PR<Id> {
        let start = self.start_span();
        self.char_('!')?;
        let id = self.end_span(start);
        self.cond_spacing_or_lf(single)?;
        let expr = self.cond_read_expr(single)?;
        Ok(self.node(id, Token::TC_Unary(Self::cond_typ(single), "!".to_string(), expr)))
    }

    fn cond_read_expr(&mut self, single: bool) -> PR<Id> {
        alt!(
            self,
            |p| p.cond_read_group(single),
            |p| p.cond_read_unary_exp(single),
            |p| p.cond_read_nullary_or_binary(single)
        )
    }

    fn cond_read_or(&mut self, single: bool) -> PR<Id> {
        self.chainl1(
            |p| p.cond_read_and(single),
            |p| p.cond_read_and_op(single),
            |p, (id, op), x, y| p.node(id, Token::TC_And(Self::cond_typ(single), op, x, y)),
        )
    }

    fn cond_read_and(&mut self, single: bool) -> PR<Id> {
        self.chainl1(
            |p| p.cond_read_term(single),
            |p| p.cond_read_or_op(single),
            |p, (id, op), x, y| p.node(id, Token::TC_Or(Self::cond_typ(single), op, x, y)),
        )
    }

    fn cond_contents(&mut self, single: bool) -> PR<Id> {
        self.cond_read_or(single)
    }
}

fn cond_ended_with(ast: &Ast, str: &str, t: Id) -> bool {
    match ast.get(t) {
        Token::T_NormalWord(s) if !s.is_empty() => match ast.get(*s.last().unwrap()) {
            Token::T_Literal(s) => s.ends_with(str),
            _ => false,
        },
        _ => false,
    }
}

fn cond_not_array_index(ast: &Ast, t: Id) -> bool {
    match ast.get(t) {
        Token::T_NormalWord(s) if s.len() >= 2 => match ast.get(s[1]) {
            Token::T_Literal(t) => t != "[",
            _ => true,
        },
        _ => true,
    }
}
