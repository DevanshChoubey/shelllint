// SPDX-License-Identifier: GPL-3.0-or-later
//! Open document text with incremental (range-based) edits.

use lsp_types::{Position, TextDocumentContentChangeEvent};

#[derive(Clone, Debug)]
pub struct Document {
    pub text: String,
    pub version: i32,
    /// File name used for shell detection and rc lookup.
    pub filename: String,
    /// True for non-file URIs (untitled buffers): no rc/editorconfig lookup.
    pub is_file: bool,
}

/// Byte offset of an LSP position (UTF-16 character). Out-of-range positions
/// clamp to the end of their line or of the text.
pub fn offset_of(text: &str, pos: Position) -> usize {
    let mut line_start = 0usize;
    if pos.line > 0 {
        let mut line = 0u32;
        let mut found = false;
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line += 1;
                if line == pos.line {
                    line_start = i + 1;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return text.len();
        }
    }
    let mut units = 0u32;
    for (i, c) in text[line_start..].char_indices() {
        if units >= pos.character || c == '\n' {
            return line_start + i;
        }
        units += c.len_utf16() as u32;
    }
    text.len()
}

impl Document {
    /// Apply one content change. A change without a range replaces the whole text.
    pub fn apply(&mut self, change: &TextDocumentContentChangeEvent) {
        match change.range {
            None => self.text = change.text.clone(),
            Some(r) => {
                let start = offset_of(&self.text, r.start);
                let end = offset_of(&self.text, r.end).max(start);
                self.text.replace_range(start..end, &change.text);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range};

    fn change(r: Option<Range>, t: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent { range: r, range_length: None, text: t.into() }
    }
    fn p(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn incremental_insert_and_delete() {
        let mut d = Document { text: "a\nbc\n".into(), version: 1, filename: "x.sh".into(), is_file: true };
        d.apply(&change(Some(Range { start: p(1, 1), end: p(1, 1) }), "XY"));
        assert_eq!(d.text, "a\nbXYc\n");
        d.apply(&change(Some(Range { start: p(0, 0), end: p(1, 2) }), ""));
        assert_eq!(d.text, "Yc\n");
    }

    #[test]
    fn utf16_positions_and_full_sync() {
        let mut d = Document { text: "\u{1F600}x".into(), version: 1, filename: "x.sh".into(), is_file: true };
        d.apply(&change(Some(Range { start: p(0, 2), end: p(0, 3) }), "y"));
        assert_eq!(d.text, "\u{1F600}y");
        d.apply(&change(None, "new"));
        assert_eq!(d.text, "new");
    }
}
