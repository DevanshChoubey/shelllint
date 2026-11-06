// SPDX-License-Identifier: GPL-3.0-only
// Tests for the Parsec engine underlying the port of ShellCheck src/ShellCheck/Parser.hs (e11f964)

use shellcheck_core::interface::Severity;
use shellcheck_parser::engine::{Fail, P, PR, ParseError};
use shellcheck_parser::testing::{self, test_environment};
use shellcheck_parser::{alt, choice};

fn run<T>(input: &str, f: impl FnOnce(&mut P<'_>) -> PR<T>) -> (PR<T>, usize, usize, usize, u32) {
    let mut p = P::new(test_environment(), "-", input);
    let r = f(&mut p);
    let ast_len = p.user.ast.len() as usize;
    let notes = p.user.parse_notes.len();
    let problems = p.sys.parse_problems.len();
    let column = p.get_position().column;
    (r, ast_len, notes, problems, column)
}

#[test]
fn alt_does_not_try_second_after_consumed_failure() {
    let (r, ..) = run("ac", |p| p.alt(|p| p.string("ab"), |p| p.string("ac")));
    assert!(matches!(r, Err(Fail::Consumed(_))), "{r:?}");
}

#[test]
fn try_allows_second() {
    let (r, ..) = run("ac", |p| p.alt(|p| p.try_(|p| p.string("ab")), |p| p.string("ac")));
    assert_eq!(r, Ok("ac".to_string()));
}

#[test]
fn user_state_rolls_back() {
    let (r, ast_len, notes, _, _) = run("ab", |p| {
        p.optional(|p| {
            p.try_(|p| {
                p.any_char()?;
                let pos = p.get_position();
                p.get_next_id_between(pos, pos);
                p.parse_note(Severity::ErrorC, 1000, "note");
                p.char_('x')
            })
        })
    });
    assert_eq!(r, Ok(()));
    assert_eq!(ast_len, 0);
    assert_eq!(notes, 0);
}

#[test]
fn system_state_does_not_roll_back() {
    let (r, _, _, problems, _) = run("ab", |p| {
        p.optional(|p| {
            p.try_(|p| {
                p.any_char()?;
                p.parse_problem(Severity::ErrorC, 1000, "problem");
                p.char_('x')
            })
        })
    });
    assert_eq!(r, Ok(()));
    assert_eq!(problems, 1);
}

#[test]
fn tab_column() {
    let (r, _, _, _, column) = run("\tx", |p| p.string("\tx"));
    assert!(r.is_ok());
    assert_eq!(column, 10);
}

#[test]
fn unicode_column() {
    let (r, _, _, _, column) = run("é", |p| p.char_('é'));
    assert!(r.is_ok());
    assert_eq!(column, 2);
}

#[test]
fn fail_message_is_kept() {
    let (r, ..) = run("", |p| p.fail::<()>("Expected thing"));
    let Err(Fail::Empty(ParseError { messages, .. })) = r else { panic!("{r:?}") };
    assert_eq!(messages, vec!["Expected thing".to_string()]);
}

#[test]
fn errors_merge_at_same_pos() {
    let (r, ..) = run("x", |p| p.alt(|p| p.fail::<()>("one"), |p| p.fail("two")));
    let Err(Fail::Empty(e)) = r else { panic!("{r:?}") };
    assert_eq!(e.messages, vec!["one".to_string(), "two".to_string()]);
}

#[test]
fn later_error_position_wins() {
    // try (char 'x' >> fail "late") <|> fail "early": the try'd error is one column further.
    let (r, ..) = run("x", |p| {
        p.alt(|p| p.try_(|p| { p.char_('x')?; p.fail::<()>("late") }), |p| p.fail("early"))
    });
    let Err(Fail::Empty(e)) = r else { panic!("{r:?}") };
    assert_eq!(e.messages, vec!["late".to_string()]);
}

#[test]
fn empty_ok_error_is_merged_into_later_failure() {
    // option () (fail "a") >> fail "b": Parsec carries "a" along with the empty success.
    let (r, ..) = run("", |p| {
        p.option((), |p| p.fail("a"))?;
        p.fail::<()>("b")
    });
    let Err(Fail::Empty(e)) = r else { panic!("{r:?}") };
    assert_eq!(e.messages, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn consumed_input_drops_earlier_errors() {
    // option () (fail "a") >> char 'x' >> fail "b": consuming 'x' resets the error.
    let (r, ..) = run("x", |p| {
        p.option((), |p| p.fail("a"))?;
        p.char_('x')?;
        p.fail::<()>("b")
    });
    assert_eq!(r.map_err(Fail::into_error).unwrap_err().messages, vec!["b".to_string()]);
}

#[test]
fn string_consumes_matched_prefix() {
    // Parsec 3.1.16's `string` fails *consumed* once its first char matched.
    let (r, ..) = run("ax", |p| p.string("ab"));
    assert!(matches!(r, Err(Fail::Consumed(_))));
    let (r, ..) = run("xb", |p| p.string("ab"));
    assert!(matches!(r, Err(Fail::Empty(_))));
}

#[test]
fn many_stops_without_consuming_and_restores_state() {
    let (r, ast_len, ..) = run("aab", |p| {
        let xs = p.many(|p| p.char_('a'))?;
        // A failing, non-consuming iteration must not leak its id allocation.
        let ys = p.many(|p| {
            let pos = p.get_position();
            p.get_next_id_between(pos, pos);
            p.char_('z')
        })?;
        Ok((xs, ys))
    });
    assert_eq!(r, Ok((vec!['a', 'a'], vec![])));
    assert_eq!(ast_len, 0);
}

#[test]
fn look_ahead_restores_input() {
    let (r, _, _, _, column) = run("ab", |p| p.look_ahead(|p| p.string("ab")));
    assert_eq!(r, Ok("ab".to_string()));
    assert_eq!(column, 1);
}

#[test]
fn eof_and_not_followed_by() {
    let (r, ..) = run("", |p| p.eof());
    assert_eq!(r, Ok(()));
    let (r, ..) = run("a", |p| p.eof());
    assert!(matches!(r, Err(Fail::Empty(_))));
    let (r, ..) = run("ab", |p| { p.char_('a')?; p.not_followed_by(|p| p.char_('c'))?; p.char_('b') });
    assert_eq!(r, Ok('b'));
}

#[test]
fn reluctantly_till_stops_at_end() {
    let (r, _, _, _, column) = run("abc;", |p| p.reluctantly_till(|p| p.any_char(), |p| p.char_('c')));
    assert_eq!(r, Ok(vec!['a', 'b']));
    assert_eq!(column, 3);
}

#[test]
fn chainl1_is_left_associative() {
    let (r, ..) = run("1-2-3", |p| {
        p.chainl1(
            |p| p.digit().map(|c| c.to_string()),
            |p| p.char_('-'),
            |_, _, x, y| format!("({x}-{y})"),
        )
    });
    assert_eq!(r, Ok("((1-2)-3)".to_string()));
}

#[test]
fn chainr1_is_right_associative() {
    let (r, ..) = run("1=2=3", |p| {
        p.chainr1(
            |p| p.digit().map(|c| c.to_string()),
            |p| p.char_('='),
            |_, _, x, y| format!("({x}={y})"),
        )
    });
    assert_eq!(r, Ok("(1=(2=3))".to_string()));
}

#[test]
fn called_leaves_context_on_consumed_failure() {
    // parsecBracket only pops the context when the body fails without consuming.
    let mut p = P::new(test_environment(), "-", "ab");
    let r = p.called("thing", |p| { p.char_('a')?; p.char_('x') });
    assert!(matches!(r, Err(Fail::Consumed(_))));
    assert_eq!(p.sys.context_stack.len(), 1);
    let mut p = P::new(test_environment(), "-", "ab");
    let r = p.called("thing", |p| p.char_('x'));
    assert!(matches!(r, Err(Fail::Empty(_))));
    assert_eq!(p.sys.context_stack.len(), 0);
}

#[test]
fn named_parser_covers_corpus_targets() {
    let corpus = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/tests.jsonl"))
        .expect("corpus/tests.jsonl");
    let mut missing = vec![];
    let mut seen = 0;
    for line in corpus.lines() {
        if !line.contains("\"kind\": \"parse\"") {
            continue;
        }
        seen += 1;
        let key = "\"target\": \"";
        let start = line.find(key).expect("parse record without target") + key.len();
        let target = &line[start..start + line[start..].find('"').unwrap()];
        if testing::named_parser(target).is_none() {
            missing.push(target.to_string());
        }
    }
    assert!(seen > 300, "only {seen} parse records found");
    assert!(missing.is_empty(), "missing named parsers: {missing:?}");
}

#[test]
fn parses_cleanly_on_ported_reader() {
    // All parser areas are ported (Phase 1 parser gate), so no reader is a stub any more.
    let f = testing::named_parser("readSimpleCommand").expect("named parser");
    assert_eq!(testing::parses_cleanly(f, "echo foo"), Some(true));
}

#[test]
fn alt_and_choice_macros() {
    let (r, ..) = run("c", |p| alt!(p, |p| p.char_('a'), |p| p.char_('b'), |p| p.char_('c')));
    assert_eq!(r, Ok('c'));
    let (r, ..) = run("d", |p| choice!(p, |p| p.char_('a'), |p| p.fail("not b")));
    let Err(Fail::Empty(e)) = r else { panic!("{r:?}") };
    assert_eq!(e.messages, vec!["not b".to_string()]);
    let (r, ..) = run("ab", |p| p.choice(["ab", "a"], |p, s| p.try_(|p| p.string(s))));
    assert_eq!(r, Ok("ab".to_string()));
}

#[test]
fn sep_by_and_many_till() {
    let (r, ..) = run("a,a,a;", |p| p.sep_by(|p| p.char_('a'), |p| p.char_(',')));
    assert_eq!(r, Ok(vec!['a', 'a', 'a']));
    let (r, ..) = run(";", |p| p.sep_by(|p| p.char_('a'), |p| p.char_(',')));
    assert_eq!(r, Ok(vec![]));
    let (r, ..) = run("xy;", |p| p.many_till(|p| p.any_char(), |p| p.char_(';')));
    assert_eq!(r, Ok(vec!['x', 'y']));
}

#[test]
#[should_panic(expected = "combinator 'many' is applied to a parser that accepts an empty string")]
fn many_of_empty_parser_panics_like_parsec() {
    let _ = run("a", |p| p.many(|p| p.option('x', |p| p.char_('z'))));
}

#[test]
fn with_stack_runs_deep_recursion() {
    fn nest(p: &mut P<'_>) -> PR<u32> {
        p.alt(|p| { p.char_('(')?; let n = nest(p)?; p.char_(')')?; Ok(n + 1) }, |_| Ok(0))
    }
    let depth = 5000;
    let input = format!("{}{}", "(".repeat(depth), ")".repeat(depth));
    let r = shellcheck_parser::engine::with_stack(256 << 20, move || run(&input, nest).0);
    assert_eq!(r, Ok(depth as u32));
}
