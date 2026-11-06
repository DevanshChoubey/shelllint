// SPDX-License-Identifier: GPL-3.0-or-later
//! Ports of the `Data.Char` predicates ShellCheck uses (GHC `GHC.Unicode`).
//!
//! GHC defines these by Unicode general category, while Rust's
//! `char::is_alphabetic` / `is_uppercase` / `is_lowercase` use derived
//! properties (Alphabetic, Uppercase, Lowercase) that add Other_Alphabetic
//! marks (e.g. Devanagari vowel signs), Other_Lowercase modifier letters
//! (U+00AA, U+02B0) and drop titlecase letters (U+01C5). Those differences
//! change parses and checks on ordinary non-Latin text, so the categories are
//! looked up in generated tables (`unicode_tables.rs`).
// DIVERGENCE: the tables come from the Unicode version of the generating
// Python (see `UNICODE_VERSION`); GHC's own version depends on the GHC release
// that built ShellCheck. Only code points assigned in between can differ.

use crate::unicode_tables::{ALPHA, LOWER, NUMBER, UPPER};

pub use crate::unicode_tables::UNICODE_VERSION;

fn in_table(table: &[(u32, u32)], c: char) -> bool {
    let c = c as u32;
    table
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `isUpper`: general category Lu or Lt.
pub fn is_upper(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_uppercase() } else { in_table(UPPER, c) }
}

/// `isLower`: general category Ll.
pub fn is_lower(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_lowercase() } else { in_table(LOWER, c) }
}

/// `isAlpha` (= `isLetter`): general category L*.
pub fn is_alpha(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphabetic() } else { in_table(ALPHA, c) }
}

/// `isLetter` (same as `isAlpha`).
pub fn is_letter(c: char) -> bool {
    is_alpha(c)
}

/// `isNumber`: general category N*.
pub fn is_number(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_digit() } else { in_table(NUMBER, c) }
}

/// `isAlphaNum`: general category L* or N*.
pub fn is_alpha_num(c: char) -> bool {
    if c.is_ascii() { c.is_ascii_alphanumeric() } else { in_table(ALPHA, c) || in_table(NUMBER, c) }
}

/// `isSpace`: `\t\n\v\f\r`, space, U+00A0, and category Zs.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t'..='\r'
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_matches_std() {
        for b in 0u8..128 {
            let c = b as char;
            assert_eq!(is_upper(c), c.is_ascii_uppercase(), "{c:?}");
            assert_eq!(is_lower(c), c.is_ascii_lowercase(), "{c:?}");
            assert_eq!(is_alpha(c), c.is_ascii_alphabetic(), "{c:?}");
            assert_eq!(is_alpha_num(c), c.is_ascii_alphanumeric(), "{c:?}");
        }
    }

    #[test]
    fn general_category_not_derived_properties() {
        // Lt is upper in GHC, not in Rust's Uppercase property.
        for c in ['\u{1C5}', '\u{1C8}', '\u{1CB}', '\u{1F2}'] {
            assert!(is_upper(c) && !is_lower(c) && is_alpha(c), "{c:?}");
        }
        // Lo / Lm letters with Other_Lowercase are not Ll.
        for c in ['\u{AA}', '\u{BA}', '\u{2B0}'] {
            assert!(!is_lower(c) && !is_upper(c) && is_alpha(c), "{c:?}");
        }
        // Mn/Mc vowel signs and So circled letters are Alphabetic but not L*.
        for c in ['\u{93E}', '\u{94D}', '\u{E34}', '\u{345}', '\u{24B6}'] {
            assert!(!is_alpha(c) && !is_alpha_num(c), "{c:?}");
        }
        // Letters and numbers in other scripts.
        for c in ['न', 'म', 'ß', 'Ж', '中'] {
            assert!(is_alpha(c) && is_alpha_num(c), "{c:?}");
        }
        for c in ['\u{966}', '\u{2160}', '\u{B2}'] {
            assert!(!is_alpha(c) && is_number(c) && is_alpha_num(c), "{c:?}");
        }
        assert!(is_upper('Ж') && is_lower('ж'));
        assert!(!is_alpha('\u{10FFFF}') && !is_alpha_num('\u{E000}'));
    }
}
