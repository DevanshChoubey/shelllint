// SPDX-License-Identifier: GPL-3.0-or-later
//
// DIVERGENCE: `--tab-width=N` is an extension; ShellCheck always echoes the
// source line with its tabs untouched.
//
// ShellCheck's columns advance a tab to the next multiple of 8, so a column in
// a comment is a column in "8-stop" space. These helpers translate it to the
// display column under an `N`-wide tab stop and expand the tabs of the echoed
// source, so that the caret line lines up with the echoed text.

/// Tab stop used by the parser when it counts columns.
const PARSER_TAB: usize = 8;

/// Expand every tab in `line` to spaces, to the next multiple of `width`.
pub fn expand_tabs(line: &str, width: usize) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    for c in line.chars() {
        if c == '\t' {
            let n = width - (col % width);
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

/// The 1-based display column, under tab stops of `width`, of the character
/// that the parser reports at 1-based column `col` of `line`. Columns past the
/// end of the line keep their distance from the end.
pub fn display_column(line: &str, col: i64, width: usize) -> i64 {
    if col < 1 {
        return col;
    }
    let target = col as usize;
    let (mut parser_col, mut shown_col) = (1usize, 1usize);
    for c in line.chars() {
        if parser_col >= target {
            break;
        }
        if c == '\t' {
            parser_col += PARSER_TAB - ((parser_col - 1) % PARSER_TAB);
            shown_col += width - ((shown_col - 1) % width);
        } else {
            parser_col += 1;
            shown_col += 1;
        }
    }
    // A column inside a tab, or past the end of the line, keeps its offset.
    (shown_col + target.saturating_sub(parser_col)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_to_stops() {
        assert_eq!(expand_tabs("\techo", 4), "    echo");
        assert_eq!(expand_tabs("ab\tc", 4), "ab  c");
        assert_eq!(expand_tabs("abcd\tc", 4), "abcd    c");
        assert_eq!(expand_tabs("\t\tx", 2), "    x");
        assert_eq!(expand_tabs("no tabs", 4), "no tabs");
    }

    #[test]
    fn columns_translate() {
        // "\techo $x": the parser puts `$x` at column 14; with width 4 the
        // echoed line is "    echo $x" and `$x` is at column 10.
        assert_eq!(display_column("\techo $x", 14, 4), 10);
        assert_eq!(display_column("\techo $x", 14, 8), 14);
        assert_eq!(display_column("echo $x", 6, 4), 6);
        assert_eq!(display_column("a\tb", 9, 4), 5);
        // Past the end of the line.
        assert_eq!(display_column("\t", 12, 4), 8);
    }
}
