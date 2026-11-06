// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 713-917:
// readArithmeticContents and its precedence ladder.
//
// The Haskell defines the ladder in a `where` block under
// `readArithmeticContents`; here each local definition is a method named
// `arith_*` (the local `spacing` is `arith_spacing`, so it does not shadow the
// global `spacing`).

use shellcheck_core::ast::{Id, Token};
use shellcheck_core::interface::Severity;

use crate::engine::{P, PR};
use crate::{alt, choice};

impl<'a> P<'a> {
    pub(crate) fn read_arithmetic_contents(&mut self) -> PR<Id> {
        self.arith_sequence()
    }

    fn arith_spacing(&mut self) -> PR<()> {
        self.many(|p| {
            alt!(
                p,
                |p| p.whitespace(),
                |p| {
                    p.try_(|p| p.string("\\\n"))?;
                    Ok('\n')
                }
            )
        })
        .map(drop)
    }

    // splitBy x ops = chainl1 x (readBinary ops)
    fn arith_split_by(&mut self, x: fn(&mut Self) -> PR<Id>, ops: &[&str]) -> PR<Id> {
        self.chainl1(
            x,
            |p| p.arith_read_binary(ops),
            |p, (id, op), l, r| p.node(id, Token::TA_Binary(op, l, r)),
        )
    }

    fn arith_read_binary(&mut self, ops: &[&str]) -> PR<(Id, String)> {
        self.arith_read_combo_op(ops)
    }

    // readComboOp op token: returns the allocated id and operator; the caller
    // builds the token (`token id op`).
    fn arith_read_combo_op(&mut self, ops: &[&str]) -> PR<(Id, String)> {
        let start = self.start_span();
        let op = self.choice(ops.iter().copied(), |p, x| {
            p.try_(|p| {
                let s = p.string(x)?;
                p.arith_fail_if_incomplete_op()?;
                Ok(s)
            })
        })?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok((id, op))
    }

    fn arith_fail_if_incomplete_op(&mut self) -> PR<()> {
        self.not_followed_by2(|p| p.one_of("&|<>="))
    }

    // Read binary minus, but also check for -lt, -gt and friends:
    fn arith_read_minus_op(&mut self) -> PR<(Id, String)> {
        let start = self.start_span();
        let pos = self.get_position();
        self.try_(|p| {
            p.char_('-')?;
            p.arith_fail_if_incomplete_op()
        })?;
        self.optional(|p| {
            let (str, alt) = p.look_ahead(|p| {
                p.choice(
                    [
                        ("lt", "<"),
                        ("gt", ">"),
                        ("le", "<="),
                        ("ge", ">="),
                        ("eq", "=="),
                        ("ne", "!="),
                    ],
                    |p, (str, alt)| {
                        p.try_(|p| {
                            p.string(str)?;
                            p.spacing1()?;
                            Ok((str, alt))
                        })
                    },
                )
            })?;
            p.parse_problem_at(
                pos,
                Severity::ErrorC,
                1106,
                &format!("In arithmetic contexts, use {} instead of -{}", alt, str),
            );
            Ok(())
        })?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok((id, "-".to_string()))
    }

    fn arith_read_array_index(&mut self) -> PR<Id> {
        let start = self.start_span();
        self.char_('[')?;
        let pos = self.get_position();
        let middle = self.read_string_for_parser(|p| p.read_arithmetic_contents())?;
        self.char_(']')?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_UnparsedIndex(pos.pos(), middle)))
    }

    fn arith_literal(&mut self, s: &str) -> PR<Id> {
        let start = self.start_span();
        self.string(s)?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::T_Literal(s.to_string())))
    }

    fn arith_read_variable(&mut self) -> PR<Id> {
        let start = self.start_span();
        let name = self.read_variable_name()?;
        let indices = self.many(|p| p.arith_read_array_index())?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok(self.node(id, Token::TA_Variable(name, indices)))
    }

    fn arith_read_expansion(&mut self) -> PR<Id> {
        let start = self.start_span();
        let pieces = self.many1(|p| {
            choice!(
                p,
                |p| p.read_single_quoted(),
                |p| p.read_double_quoted(),
                |p| p.read_normal_dollar(),
                |p| p.read_braced(),
                |p| p.read_unquoted_back_ticked(),
                |p| p.arith_literal("#"),
                |p| p.read_normal_literal("+-*/=%^,]?:")
            )
        })?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok(self.node(id, Token::TA_Expansion(pieces)))
    }

    fn arith_read_group(&mut self) -> PR<Id> {
        let start = self.start_span();
        self.char_('(')?;
        let s = self.arith_sequence()?;
        self.char_(')')?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        Ok(self.node(id, Token::TA_Parenthesis(s)))
    }

    fn arith_read_arith_term(&mut self) -> PR<Id> {
        alt!(
            self,
            |p| p.arith_read_group(),
            |p| p.arith_read_variable(),
            |p| p.arith_read_expansion()
        )
    }

    fn arith_sequence(&mut self) -> PR<Id> {
        self.arith_spacing()?;
        let start = self.start_span();
        let l = self.sep_by(
            |p| p.arith_read_assignment(),
            |p| {
                p.char_(',')?;
                p.arith_spacing()
            },
        )?;
        let id = self.end_span(start);
        Ok(self.node(id, Token::TA_Sequence(l)))
    }

    fn arith_read_assignment(&mut self) -> PR<Id> {
        self.chainr1(
            |p| p.arith_read_trinary(),
            |p| p.arith_read_assignment_op(),
            |p, (id, op), l, r| p.node(id, Token::TA_Assignment(op, l, r)),
        )
    }

    fn arith_read_assignment_op(&mut self) -> PR<(Id, String)> {
        self.arith_read_combo_op(&["=", "*=", "/=", "%=", "+=", "-=", "<<=", ">>=", "&=", "^=", "|="])
    }

    fn arith_read_trinary(&mut self) -> PR<Id> {
        let x = self.arith_read_logical_or()?;
        alt!(
            self,
            |p| {
                let start = p.start_span();
                p.string("?")?;
                p.arith_spacing()?;
                let y = p.arith_read_trinary()?;
                p.string(":")?;
                p.arith_spacing()?;
                let z = p.arith_read_trinary()?;
                let id = p.end_span(start);
                Ok(p.node(id, Token::TA_Trinary(x, y, z)))
            },
            |_| Ok(x)
        )
    }

    fn arith_read_logical_or(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_logical_and(), &["||"])
    }
    fn arith_read_logical_and(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_bit_or(), &["&&"])
    }
    fn arith_read_bit_or(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_bit_xor(), &["|"])
    }
    fn arith_read_bit_xor(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_bit_and(), &["^"])
    }
    fn arith_read_bit_and(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_equated(), &["&"])
    }
    fn arith_read_equated(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_compared(), &["==", "!="])
    }
    fn arith_read_compared(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_shift(), &["<=", ">=", "<", ">"])
    }
    fn arith_read_shift(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_addition(), &["<<", ">>"])
    }
    fn arith_read_addition(&mut self) -> PR<Id> {
        self.chainl1(
            |p| p.arith_read_multiplication(),
            |p| alt!(p, |p| p.arith_read_binary(&["+"]), |p| p.arith_read_minus_op()),
            |p, (id, op), l, r| p.node(id, Token::TA_Binary(op, l, r)),
        )
    }
    fn arith_read_multiplication(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_exponential(), &["*", "/", "%"])
    }
    fn arith_read_exponential(&mut self) -> PR<Id> {
        self.arith_split_by(|p| p.arith_read_any_negated(), &["**"])
    }

    fn arith_read_any_negated(&mut self) -> PR<Id> {
        alt!(self, |p| p.arith_read_negated(), |p| p.arith_read_any_signed())
    }

    fn arith_read_negated(&mut self) -> PR<Id> {
        let start = self.start_span();
        let op = self.one_of("!~")?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        let x = self.arith_read_any_negated()?;
        Ok(self.node(id, Token::TA_Unary(op.to_string(), x)))
    }

    fn arith_read_any_signed(&mut self) -> PR<Id> {
        alt!(self, |p| p.arith_read_signed(), |p| p.arith_read_anycremented())
    }

    fn arith_read_signed(&mut self) -> PR<Id> {
        let start = self.start_span();
        let op = self.choice("+-".chars(), |p, c| {
            p.try_(|p| {
                p.char_(c)?;
                p.not_followed_by2(|p| p.char_(c))?;
                p.arith_spacing()?;
                Ok(c)
            })
        })?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        let x = self.arith_read_anycremented()?;
        Ok(self.node(id, Token::TA_Unary(op.to_string(), x)))
    }

    fn arith_read_anycremented(&mut self) -> PR<Id> {
        alt!(
            self,
            |p| p.arith_read_normal_or_postfix_incremented(),
            |p| p.arith_read_prefix_incremented()
        )
    }

    fn arith_read_prefix_incremented(&mut self) -> PR<Id> {
        let start = self.start_span();
        let op = self.try_(|p| alt!(p, |p| p.string("++"), |p| p.string("--")))?;
        let id = self.end_span(start);
        self.arith_spacing()?;
        let x = self.arith_read_arith_term()?;
        Ok(self.node(id, Token::TA_Unary(format!("{}|", op), x)))
    }

    fn arith_read_normal_or_postfix_incremented(&mut self) -> PR<Id> {
        let x = self.arith_read_arith_term()?;
        self.arith_spacing()?;
        alt!(
            self,
            |p| {
                let start = p.start_span();
                let op = p.try_(|p| alt!(p, |p| p.string("++"), |p| p.string("--")))?;
                let id = p.end_span(start);
                p.arith_spacing()?;
                Ok(p.node(id, Token::TA_Unary(format!("|{}", op), x)))
            },
            |_| Ok(x)
        )
    }
}
