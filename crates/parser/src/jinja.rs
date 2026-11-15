// DIVERGENCE: shelllint extension (`--template=jinja`), not in ShellCheck.
//
// A Jinja template is shell text with `{{ expr }}`, `{% stmt %}` and
// `{# comment #}` markup. This pass replaces the markup with shell-neutral
// text before parsing and keeps every character and every newline in place,
// so line and column numbers stay exact.

/// Does this file name mean a Jinja template (`*.j2`, `*.jinja`, `*.jinja2`)?
pub fn is_template_name(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    [".j2", ".jinja", ".jinja2"].iter().any(|ext| lower.ends_with(ext))
}

/// Is there a `# shellcheck template=jinja` directive before the first
/// command? Only blank lines, the shebang and comments are looked at.
pub fn has_template_directive(src: &str) -> bool {
    for (n, line) in src.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || (n == 0 && t.starts_with("#!")) {
            continue;
        }
        let Some(rest) = t.strip_prefix('#') else { return false };
        let Some(rest) = rest.trim_start().strip_prefix("shellcheck") else { continue };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        for tok in rest.split_whitespace() {
            if tok.starts_with('#') {
                break;
            }
            if matches!(tok, "template=jinja" | "template=\"jinja\"" | "template='jinja'") {
                return true;
            }
        }
    }
    false
}

/// Blank out the markup of a Jinja template. Every character inside markup
/// becomes exactly one ASCII fill character (`_` for `{{ }}`, a space for
/// `{% %}` and `{# #}`), so the result has the same number of characters as
/// `src` (columns count characters), and every `\n` stays where it was.
pub fn mask_jinja(src: &str) -> String {
    mask_with_ranges(src).0
}

/// `mask_jinja`, plus the blanked runs as half-open character index ranges.
fn mask_with_ranges(src: &str) -> (String, Vec<(usize, usize)>) {
    let b: Vec<char> = src.chars().collect();
    let mut out = b.clone();
    let mut runs = vec![];
    let mut i = 0;
    while i + 1 < b.len() {
        let (close, fill): ([char; 2], char) = match (b[i], b[i + 1]) {
            ('{', '{') => (['}', '}'], '_'),
            ('{', '%') => (['%', '}'], ' '),
            ('{', '#') => (['#', '}'], ' '),
            _ => {
                i += 1;
                continue;
            }
        };
        let end = find(&b, &close, i + 2).map_or(b.len(), |p| p + 2);
        blank(&mut out, &mut runs, i, end, fill);
        if b[i + 1] == '%' && end <= b.len() && is_tag(&b[i + 2..end.saturating_sub(2).max(i + 2)], "raw") {
            // Raw content stays as written, up to the matching endraw tag.
            let mut j = end;
            loop {
                let Some(s) = find(&b, &['{', '%'], j) else {
                    i = b.len();
                    break;
                };
                let e = find(&b, &['%', '}'], s + 2).map_or(b.len(), |p| p + 2);
                if is_tag(&b[s + 2..e.saturating_sub(2).max(s + 2)], "endraw") {
                    blank(&mut out, &mut runs, s, e, ' ');
                    i = e;
                    break;
                }
                j = e;
            }
            continue;
        }
        i = end;
    }
    (out.into_iter().collect(), runs)
}

/// The masked markup of `src`, as half-open `((line, col), (line, col))`
/// ranges. Lines and columns are 1 based and tabs advance to the next
/// multiple of 8, like the positions in messages and fixes.
pub fn markup_ranges(src: &str) -> Vec<((i64, i64), (i64, i64))> {
    let runs = mask_with_ranges(src).1;
    let mut pos = Vec::with_capacity(src.len() + 1);
    let (mut line, mut col) = (1i64, 1i64);
    for c in src.chars() {
        pos.push((line, col));
        match c {
            '\n' => {
                line += 1;
                col = 1;
            }
            '\t' => col = (col - 1) / 8 * 8 + 9,
            _ => col += 1,
        }
    }
    pos.push((line, col));
    runs.into_iter().map(|(s, e)| (pos[s], pos[e])).collect()
}

/// `x.sh.j2` -> `x.sh`: drop one trailing template extension.
pub fn strip_template_ext(filename: &str) -> &str {
    for ext in [".jinja2", ".jinja", ".j2"] {
        let n = filename.len().wrapping_sub(ext.len());
        if filename.len() > ext.len() && filename.is_char_boundary(n) && filename[n..].eq_ignore_ascii_case(ext) {
            return &filename[..n];
        }
    }
    filename
}

fn find(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

fn blank(out: &mut [char], runs: &mut Vec<(usize, usize)>, start: usize, end: usize, fill: char) {
    runs.push((start, end));
    for c in &mut out[start..end] {
        if *c != '\n' {
            *c = fill;
        }
    }
}

/// Is the inside of a `{% … %}` tag just `name`, allowing `-` or `+` whitespace control?
fn is_tag(inner: &[char], name: &str) -> bool {
    let s: String = inner.iter().collect();
    s.trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '+') == name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_expr() {
        assert_eq!(mask_jinja("a={{ x }}"), "a=_______");
    }

    #[test]
    fn mask_stmt_keeps_newlines() {
        assert_eq!(mask_jinja("a\n{% if x\n and y %}\nb"), "a\n       \n         \nb");
    }

    #[test]
    fn mask_comment() {
        assert_eq!(mask_jinja("{# note #}echo"), "          echo");
    }

    #[test]
    fn raw_block_kept() {
        let s = "{% raw %}echo {{ x }}{% endraw %}!";
        assert_eq!(mask_jinja(s), format!("{}echo {{{{ x }}}}{}!", " ".repeat(9), " ".repeat(12)));
    }

    #[test]
    fn unterminated_masked_to_eof() {
        assert_eq!(mask_jinja("a {{ b\nc"), "a ____\n_");
        assert_eq!(mask_jinja("a {% b\nc"), "a     \n ");
        assert_eq!(mask_jinja("a {# b"), "a     ");
    }

    #[test]
    fn utf8_char_count_preserved() {
        let s = "x={{ caf\u{e9} \u{1F600} }} y\u{e9}";
        let m = mask_jinja(s);
        assert_eq!(m.chars().count(), s.chars().count());
        assert_eq!(m, "x=____________ y\u{e9}");
        assert!(!m.is_ascii() && m[..15].is_ascii());
    }

    #[test]
    fn same_length_property() {
        for s in [
            "NAME={{ app_name }}\nport=\"{{ port | default(80) }}\"\n",
            "{% if a %}\necho x\n{% endif %}\n",
            "{# c #}\n{# multi\nline #}\necho\n",
            "{% raw %}{{ keep }}{% endraw %}\n{{ a }}\n",
            "echo {{ \u{e9}\n}} {% for i in x %}\u{1F600}",
        ] {
            let m = mask_jinja(s);
            assert_eq!(m.chars().count(), s.chars().count(), "{s:?}");
            let nl = |t: &str| t.chars().enumerate().filter(|(_, c)| *c == '\n').map(|(i, _)| i).collect::<Vec<_>>();
            assert_eq!(nl(&m), nl(s), "{s:?}");
        }
    }

    #[test]
    fn markup_ranges_and_ext() {
        let s = "a\t{{ x }}\nb {# c\nd #}";
        assert_eq!(markup_ranges(s), vec![((1, 9), (1, 16)), ((2, 3), (3, 5))]);
        assert_eq!(strip_template_ext("x.sh.j2"), "x.sh");
        assert_eq!(strip_template_ext("x.SH.Jinja2"), "x.SH");
        assert_eq!(strip_template_ext("x.sh"), "x.sh");
        assert_eq!(strip_template_ext(".j2"), ".j2");
    }

    #[test]
    fn names_and_directive() {
        assert!(is_template_name("x.sh.j2") && is_template_name("a.JINJA") && is_template_name("a.jinja2"));
        assert!(!is_template_name("x.sh") && !is_template_name("j2"));
        assert!(has_template_directive("#!/bin/bash\n# shellcheck template=jinja\necho"));
        assert!(has_template_directive("# shellcheck disable=SC2086 template=jinja\n"));
        assert!(!has_template_directive("echo\n# shellcheck template=jinja\n"));
        assert!(!has_template_directive("# template=jinja\n"));
        assert!(has_template_directive("# shellcheck template=\"jinja\"\n"));
        assert!(has_template_directive("# shellcheck template='jinja'\n"));
    }
}
