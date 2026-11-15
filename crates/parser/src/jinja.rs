// DIVERGENCE: shelllint extension (`--template=jinja`), not in ShellCheck.
//
// A Jinja template is shell text with `{{ expr }}`, `{% stmt %}` and
// `{# comment #}` markup. This pass replaces the markup with shell-neutral
// text before parsing and keeps every byte offset and every newline, so line
// and column numbers stay exact.

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
            if tok == "template=jinja" {
                return true;
            }
        }
    }
    false
}

/// Blank out the markup of a Jinja template. The result has the same byte
/// length as `src`, and every `\n` stays where it was.
pub fn mask_jinja(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i + 1 < b.len() {
        let (close, fill): (&[u8], u8) = match (b[i], b[i + 1]) {
            (b'{', b'{') => (b"}}", b'_'),
            (b'{', b'%') => (b"%}", b' '),
            (b'{', b'#') => (b"#}", b' '),
            _ => {
                i += 1;
                continue;
            }
        };
        let end = find(b, close, i + 2).map_or(b.len(), |p| p + 2);
        blank(&mut out, i, end, fill);
        if b[i + 1] == b'%' && end <= b.len() && is_tag(&b[i + 2..end.saturating_sub(2).max(i + 2)], b"raw") {
            // Raw content stays as written, up to the matching endraw tag.
            let mut j = end;
            loop {
                let Some(s) = find(b, b"{%", j) else {
                    i = b.len();
                    break;
                };
                let e = find(b, b"%}", s + 2).map_or(b.len(), |p| p + 2);
                if is_tag(&b[s + 2..e.saturating_sub(2).max(s + 2)], b"endraw") {
                    blank(&mut out, s, e, b' ');
                    i = e;
                    break;
                }
                j = e;
            }
            continue;
        }
        i = end;
    }
    // Only ASCII bytes were written, over whole markup runs, so this is valid.
    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

fn blank(out: &mut [u8], start: usize, end: usize, fill: u8) {
    for byte in &mut out[start..end] {
        if *byte != b'\n' {
            *byte = fill;
        }
    }
}

/// Is the inside of a `{% … %}` tag just `name`, allowing `-` or `+` whitespace control?
fn is_tag(inner: &[u8], name: &[u8]) -> bool {
    let s = String::from_utf8_lossy(inner);
    s.trim_matches(|c: char| c.is_whitespace() || c == '-' || c == '+').as_bytes() == name
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
    fn utf8_length_preserved() {
        let s = "x={{ caf\u{e9} \u{1F600} }} y\u{e9}";
        let m = mask_jinja(s);
        assert_eq!(m.len(), s.len());
        assert!(m.ends_with(" y\u{e9}"));
        assert!(m.starts_with("x=_"));
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
            assert_eq!(m.len(), s.len(), "{s:?}");
            let nl = |t: &str| t.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i).collect::<Vec<_>>();
            assert_eq!(nl(&m), nl(s), "{s:?}");
        }
    }

    #[test]
    fn names_and_directive() {
        assert!(is_template_name("x.sh.j2") && is_template_name("a.JINJA") && is_template_name("a.jinja2"));
        assert!(!is_template_name("x.sh") && !is_template_name("j2"));
        assert!(has_template_directive("#!/bin/bash\n# shellcheck template=jinja\necho"));
        assert!(has_template_directive("# shellcheck disable=SC2086 template=jinja\n"));
        assert!(!has_template_directive("echo\n# shellcheck template=jinja\n"));
        assert!(!has_template_directive("# template=jinja\n"));
    }
}
