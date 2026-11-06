// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck src/ShellCheck/Parser.hs (e11f964), lines 1836-2053:
// here documents, pending here documents, IO redirections, here strings.

use shellcheck_analyzer::astlib::e4m;
use shellcheck_core::ast::{Dashed, Id, Quoted, Token};
use shellcheck_core::interface::Severity::ErrorC;
use shellcheck_core::prelude::please_report;

use crate::engine::{P, PR, SourcePos};
use crate::state::HereDocContext;
use crate::{alt, choice};

impl<'a> P<'a> {
    pub(crate) fn read_here_doc(&mut self) -> PR<Id> {
        self.called("here document", |p| {
            let pos = p.get_position();
            p.try_(|p| p.string("<<"))?;
            let dashed = p.alt(|p| p.char_('-').map(|_| Dashed::Dashed), |_| Ok(Dashed::Undashed))?;
            let sp = p.spacing()?;
            p.optional(|p| {
                p.try_(|p| p.look_ahead(|p| p.char_('(')))?;
                let message = format!("Shells are space sensitive. Use '< <(cmd)', not '<<{sp}(cmd)'.");
                p.parse_problem_at(pos, ErrorC, 1038, &message);
                Ok(())
            })?;
            let start = p.start_span();
            let (quoted, end_token) = here_doc_read_token(p)?;
            let hid = p.end_span(start);

            // add empty tokens for now, read the rest in readPendingHereDocs
            let doc = p.node(hid, Token::T_HereDoc(dashed, quoted, end_token.clone(), vec![]));
            p.add_pending_here_doc(hid, dashed, quoted, &end_token);
            Ok(doc)
        })
    }

    // Its callers (linefeed, readScript) are still stubs.
    #[allow(dead_code)] // remove once area_spacing / area_toplevel are ported
    pub(crate) fn read_pending_here_docs(&mut self) -> PR<()> {
        let docs = self.pop_pending_here_docs();
        for doc in docs {
            pending_read_doc(self, doc)?;
        }
        Ok(())
    }

    pub(crate) fn read_filename(&mut self) -> PR<Id> {
        self.read_normal_word()
    }

    pub(crate) fn read_io_file_op(&mut self) -> PR<Id> {
        choice!(
            self,
            |p: &mut Self| p.g_dgreat(),
            |p: &mut Self| p.g_lessgreat(),
            |p: &mut Self| p.g_greatand(),
            |p: &mut Self| p.g_lessand(),
            |p: &mut Self| p.g_clobber(),
            |p: &mut Self| p.redir_token('<', |p, id| p.node(id, Token::T_Less)),
            |p: &mut Self| p.redir_token('>', |p, id| p.node(id, Token::T_Greater)),
        )
    }

    pub(crate) fn read_io_duplicate(&mut self) -> PR<Id> {
        self.try_(|p| {
            let start = p.start_span();
            let op = p.alt(|p| p.g_greatand(), |p| p.g_lessand())?;
            let target = p.alt(|p| p.read_io_variable(), digits_and_or_dash)?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_IoDuplicate(op, target)))
        })
    }

    pub(crate) fn read_io_file(&mut self) -> PR<Id> {
        self.called("redirection", |p| {
            let start = p.start_span();
            let op = p.read_io_file_op()?;
            p.spacing()?;
            let file = p.read_filename()?;
            let id = p.end_span(start);
            Ok(p.node(id, Token::T_IoFile(op, file)))
        })
    }

    pub(crate) fn read_io_variable(&mut self) -> PR<String> {
        self.try_(|p| {
            p.char_('{')?;
            let x = p.read_variable_name()?;
            p.char_('}')?;
            Ok(format!("{{{x}}}"))
        })
    }

    pub(crate) fn read_io_source(&mut self) -> PR<String> {
        self.try_(|p| {
            let x = alt!(
                p,
                |p: &mut Self| p.string("&"),
                |p: &mut Self| p.read_io_variable(),
                |p: &mut Self| p.many(|p| p.digit()).map(|ds| ds.into_iter().collect()),
            )?;
            p.look_ahead(|p| p.alt(|p| p.read_io_file_op().map(drop), |p| p.string("<<").map(drop)))?;
            Ok(x)
        })
    }

    pub(crate) fn read_io_redirect(&mut self) -> PR<Id> {
        let start = self.start_span();
        let n = self.read_io_source()?;
        let redir = alt!(
            self,
            |p: &mut Self| p.read_here_string(),
            |p: &mut Self| p.read_here_doc(),
            |p: &mut Self| p.read_io_duplicate(),
            |p: &mut Self| p.read_io_file(),
        )?;
        let id = self.end_span(start);
        self.skip_annotation_and_warn()?;
        self.spacing()?;
        Ok(self.node(id, Token::T_FdRedirect(n, redir)))
    }

    pub(crate) fn read_here_string(&mut self) -> PR<Id> {
        self.called("here string", |p| {
            let start = p.start_span();
            p.try_(|p| p.string("<<<"))?;
            let id = p.end_span(start);
            p.spacing()?;
            let word = p.read_normal_word()?;
            Ok(p.node(id, Token::T_HereString(word)))
        })
    }
}

//--------- readHereDoc's `where` clause

/// `unquote`
fn here_doc_unquote(s: &str) -> (Quoted, String) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= 1 {
        return (Quoted::Unquoted, s.to_string());
    }
    let cl = chars[0];
    let cr = chars[chars.len() - 1];
    if cr == cl && "\"'".contains(cl) {
        (Quoted::Quoted, chars[1..chars.len() - 1].iter().collect())
    } else if s.contains('\\') {
        (Quoted::Quoted, s.chars().filter(|&c| c != '\\').collect())
    } else {
        (Quoted::Unquoted, s.to_string())
    }
}

/// `readToken`
// Fun fact: bash considers << foo"" quoted, but not << <("foo").
fn here_doc_read_token(p: &mut P<'_>) -> PR<(Quoted, String)> {
    let str = p.read_string_for_parser(|p| p.read_normal_word())?;
    // A here doc actually works with \r\n because the \r becomes part of the token
    let crstr = p.alt(
        |p| {
            p.carriage_return()?;
            Ok(format!("{str}\r"))
        },
        |_| Ok(str.clone()),
    )?;
    Ok(here_doc_unquote(&crstr))
}

//--------- readPendingHereDocs's `where` clause

/// `readDoc`
fn pending_read_doc(p: &mut P<'_>, doc: HereDocContext) -> PR<()> {
    let HereDocContext::HereDocPending(id, dashed, quoted, end_token, ctx) = doc;
    p.swap_context(ctx, |p| {
        let doc_start_pos = p.get_position();
        let (terminated, was_warned, lines) = pending_read_doc_lines(p, dashed, &end_token)?;
        let doc_end_pos = p.get_position();
        let here_data = haskell_unlines(&lines);
        if !terminated {
            if !was_warned {
                pending_debug_here_doc(p, id, &end_token, &here_data);
            }
            return p.fail("Here document was not correctly terminated");
        }
        let list = pending_parse_here_data(p, quoted, (doc_start_pos, doc_end_pos), &here_data)?;
        p.add_to_here_doc_map(id, list);
        Ok(())
    })
}

/// `readDocLines`: Read the lines making up the here doc. Returns (IsTerminated, Lines)
// DIVERGENCE: Haskell recurses once per line; the recursive call is the last
// step of the sequence, so it is unrolled into a loop with the same
// consumption, state and error behaviour.
fn pending_read_doc_lines(p: &mut P<'_>, dashed: Dashed, end_token: &str) -> PR<(bool, bool, Vec<String>)> {
    let mut warned = false;
    let mut lines = Vec::new();
    loop {
        let pos = p.get_position();
        let str = pending_raw_line(p)?;
        let is_eof = p.option(false, |p| p.eof().map(|_| true))?;
        let (is_end, was_warned) = p.sub_parse(pos, |p| pending_check_end(p, dashed, end_token), &str)?;
        warned = warned || was_warned;
        if is_end {
            return Ok((true, warned, lines));
        }
        lines.push(str);
        if is_eof {
            return Ok((false, warned, lines));
        }
    }
}

/// `checkEnd`: Check if this is the actual end, or a plausible false end
fn pending_check_end(p: &mut P<'_>, dashed: Dashed, end_token: &str) -> PR<(bool, bool)> {
    p.option((false, false), |p| {
        p.try_(|p| {
            // Match what's basically '^( *)token( *)(.*)$'
            let leading_space_pos = p.get_position();
            let leading_space = p.reluctantly_till(|p| p.linewhitespace(), |p| p.string(end_token))?;
            p.string(end_token)?;
            let trailing_space_pos = p.get_position();
            let trailing_space = p.many(|p| p.linewhitespace())?;
            let trailer_pos = p.get_position();
            let trailer = p.many(|p| p.any_char())?;

            let leading_spaces_are_tabs = leading_space.iter().all(|&c| c == '\t');
            let there_is_no_trailer = trailing_space.is_empty() && trailer.is_empty();
            let leader_is_ok = leading_space.is_empty() || dashed == Dashed::Dashed && leading_spaces_are_tabs;
            let trailer_start = trailer.first().copied().unwrap_or('\0');
            let has_trailing_space = !trailing_space.is_empty();
            let has_trailer = !trailer.is_empty();
            let ppt = |p: &mut P<'_>, code, msg: &str| p.parse_problem_at(trailer_pos, ErrorC, code, msg);

            if leader_is_ok && there_is_no_trailer {
                return Ok((true, false));
            }
            let found_cause = (false, true);
            let skip_line = (false, false);
            // This may be intended as an end token. Debug why it isn't.
            if trailer_start == ')' {
                ppt(p, 1119, "Add a linefeed between end token and terminating ')'.");
                Ok(found_cause)
            } else if trailer_start == '#' {
                ppt(p, 1120, "No comments allowed after here-doc token. Comment the next line instead.");
                Ok(found_cause)
            } else if ";>|&".contains(trailer_start) {
                ppt(p, 1121, "Add ;/& terminators (and other syntax) on the line with the <<, not here.");
                Ok(found_cause)
            } else if has_trailing_space && has_trailer {
                ppt(
                    p,
                    1122,
                    "Nothing allowed after end token. To continue a command, put it on the line with the <<.",
                );
                Ok(found_cause)
            } else if leader_is_ok && has_trailing_space && !has_trailer {
                p.parse_problem_at(trailing_space_pos, ErrorC, 1118, "Delete whitespace after the here-doc end token.");
                // Parse as if it's the actual end token. Will koala_man regret this once again?
                Ok((true, true))
            } else if !has_trailing_space && has_trailer {
                // The end token is just a prefix
                Ok(skip_line)
            } else if has_trailer {
                panic!("{}", please_report("unexpected heredoc trailer"))
            }
            // The following cases assume no trailing text:
            else if dashed == Dashed::Undashed && !leading_space.is_empty() {
                p.parse_problem_at(
                    leading_space_pos,
                    ErrorC,
                    1039,
                    "Remove indentation before end token (or use <<- and indent with tabs).",
                );
                Ok(found_cause)
            } else if dashed == Dashed::Dashed && !leading_spaces_are_tabs {
                p.parse_problem_at(leading_space_pos, ErrorC, 1040, "When using <<-, you can only indent with tabs.");
                Ok(found_cause)
            } else {
                Ok(skip_line)
            }
        })
    })
}

/// `rawLine`
fn pending_raw_line(p: &mut P<'_>) -> PR<String> {
    let c = p.many(|p| p.none_of("\n"))?;
    p.alt(|p| p.char_('\n').map(drop), |p| p.eof())?;
    Ok(c.into_iter().collect())
}

/// `parseHereData`
fn pending_parse_here_data(
    p: &mut P<'_>,
    quoted: Quoted,
    (start, end): (SourcePos, SourcePos),
    here_data: &str,
) -> PR<Vec<Id>> {
    match quoted {
        Quoted::Quoted => {
            let id = p.get_next_id_between(start, end);
            Ok(vec![p.node(id, Token::T_Literal(here_data.to_string()))])
        }
        Quoted::Unquoted => p.sub_parse(start, pending_read_here_data, here_data),
    }
}

/// `readHereData`
fn pending_read_here_data(p: &mut P<'_>) -> PR<Vec<Id>> {
    p.many(|p| p.alt(|p| p.double_quoted_part(), pending_read_here_literal))
}

/// `readHereLiteral`
fn pending_read_here_literal(p: &mut P<'_>) -> PR<Id> {
    let start = p.start_span();
    let chars = p.many1(|p| p.none_of("`$\\"))?;
    let id = p.end_span(start);
    Ok(p.node(id, Token::T_Literal(chars.into_iter().collect())))
}

/// `debugHereDoc`
fn pending_debug_here_doc(p: &mut P<'_>, token_id: Id, end_token: &str, doc: &str) {
    if doc.contains(end_token) {
        p.parse_problem_at_id(
            token_id,
            ErrorC,
            1041,
            &format!("Found '{}' further down, but not on a separate line.", e4m(end_token)),
        );
        for line in haskell_lines(doc) {
            if line.contains(end_token) {
                p.parse_problem_at_id(
                    token_id,
                    ErrorC,
                    1042,
                    &format!("Close matches include '{}' (!= '{}').", e4m(line), e4m(end_token)),
                );
            }
        }
    } else if haskell_lower(doc).contains(&haskell_lower(end_token)) {
        p.parse_problem_at_id(
            token_id,
            ErrorC,
            1043,
            &format!("Found {} further down, but with wrong casing.", e4m(end_token)),
        );
    } else {
        p.parse_problem_at_id(
            token_id,
            ErrorC,
            1044,
            &format!("Couldn't find end token `{}' in the here document.", e4m(end_token)),
        );
    }
}

//--------- readIoDuplicate's `where` clause

/// `digitsAndOrDash`: either digits with optional dash, or a required dash
fn digits_and_or_dash(p: &mut P<'_>) -> PR<String> {
    let str: String = p.many(|p| p.digit())?.into_iter().collect();
    let dash = if str.is_empty() { p.string("-")? } else { p.option(String::new(), |p| p.string("-"))? };
    Ok(str + &dash)
}

//--------- Haskell Prelude helpers

/// `lines`: split on '\n'; a trailing '\n' does not start another line.
/// (Unlike `str::lines`, a '\r' before '\n' is kept.)
fn haskell_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![];
    }
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.split('\n').collect()
}

/// `unlines`: every line followed by '\n'.
fn haskell_unlines(lines: &[String]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// `map toLower`
// DIVERGENCE: Haskell's `toLower` is the simple (1:1) case mapping; std only
// offers the full mapping. Taking the first char of the full mapping equals
// the simple mapping for every char (only U+0130 maps to several chars, and
// its first is its simple mapping 'i').
fn haskell_lower(s: &str) -> String {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}
