// SPDX-License-Identifier: GPL-3.0-or-later
//! Conversions between ShellCheck positions and LSP positions.
//!
//! ShellCheck columns are 1-based with tabs advancing to the next multiple of
//! 8 (plus 1); its end columns are exclusive. LSP characters are UTF-16 code
//! units, 0-based.

use lsp_types::Position;

/// Index of the char at virtual (tab-expanded, 1-based) `col`. Positions past
/// the end of the line map past its last char.
pub fn virtual_col_to_index(chars: &[char], col: i64) -> usize {
    let mut v: i64 = 1;
    for (i, &c) in chars.iter().enumerate() {
        if v >= col {
            return i;
        }
        v = if c == '\t' { v + 8 - ((v - 1) % 8) } else { v + 1 };
    }
    chars.len() + (col - v).max(0) as usize
}

/// ShellCheck `(line, column)` to an LSP position. `lines` are the document's
/// lines without the terminating newline.
pub fn to_lsp(lines: &[&str], line: i64, col: i64) -> Position {
    if lines.is_empty() {
        return Position { line: 0, character: 0 };
    }
    let li = (line - 1).clamp(0, lines.len() as i64 - 1) as usize;
    let chars: Vec<char> = lines[li].chars().collect();
    let idx = virtual_col_to_index(&chars, col);
    let units: usize = chars[..idx.min(chars.len())].iter().map(|c| c.len_utf16()).sum();
    let extra = idx.saturating_sub(chars.len());
    Position { line: li as u32, character: (units + extra) as u32 }
}

/// UTF-16 length of a line.
pub fn line_utf16_len(line: &str) -> u32 {
    line.chars().map(|c| c.len_utf16() as u32).sum()
}

/// Split a document into lines, keeping no terminators.
pub fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_to_stops_of_eight() {
        // "\tx": x sits at virtual column 9.
        let chars: Vec<char> = "\tx".chars().collect();
        assert_eq!(virtual_col_to_index(&chars, 9), 1);
        assert_eq!(virtual_col_to_index(&chars, 1), 0);
        // Past the end.
        // Column 12 is two columns past the 'x' (at 9): index 2 + 2.
        assert_eq!(virtual_col_to_index(&chars, 12), 4);
    }

    #[test]
    fn utf16_units_for_astral_chars() {
        // "😀a$": '$' is at column 3 and character 3 (emoji takes 2 units).
        let lines = ["\u{1F600}a$"];
        assert_eq!(to_lsp(&lines, 1, 3), Position { line: 0, character: 2 + 1 });
    }

    #[test]
    fn exclusive_end_column() {
        // "echo $foo": "$foo" spans columns 6..9, end is exclusive at 10.
        let lines = ["echo $foo"];
        assert_eq!(to_lsp(&lines, 1, 6).character, 5);
        assert_eq!(to_lsp(&lines, 1, 10).character, 9);
    }
}
