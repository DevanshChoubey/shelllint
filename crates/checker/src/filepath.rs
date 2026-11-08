// SPDX-License-Identifier: GPL-3.0-or-later
//! The System.FilePath functions the checker and its front ends use, with
//! GHC's per-platform behaviour: on Windows both `/` and `\` separate, `\` is
//! what gets written, and a path may start with a drive.

/// `pathSeparator`
pub const PATH_SEPARATOR: char = std::path::MAIN_SEPARATOR;

/// `isPathSeparator`
pub fn is_path_separator(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

fn is_name_boundary(c: char) -> bool {
    is_path_separator(c) || (cfg!(windows) && c == ':')
}

fn is_bare_drive(p: &str) -> bool {
    cfg!(windows) && p.len() == 2 && p.as_bytes()[0].is_ascii_alphabetic() && p.ends_with(':')
}

/// `dropFileName`, except that a path with no directory gives `""`, not `./`.
pub fn drop_file_name(path: &str) -> &str {
    match path.rfind(is_name_boundary) {
        Some(i) => &path[..=i],
        None => "",
    }
}

/// `takeFileName`
pub fn take_file_name(path: &str) -> &str {
    &path[drop_file_name(path).len()..]
}

/// `takeDirectory`: `dropTrailingPathSeparator . dropFileName`, "." if empty.
pub fn take_directory(path: &str) -> String {
    let dir = drop_file_name(path);
    // dropTrailingPathSeparator keeps a root: a path of only separators, or a
    // drive's own separator.
    let trimmed = dir.trim_end_matches(is_path_separator);
    let res =
        if trimmed.is_empty() || is_bare_drive(trimmed) { &dir[..(trimmed.len() + 1).min(dir.len())] } else { trimmed };
    if res.is_empty() { ".".to_string() } else { res.to_string() }
}

/// `</>`
pub fn combine(a: &str, b: &str) -> String {
    let rooted = b.starts_with(is_path_separator) || b.get(..2).is_some_and(is_bare_drive);
    if rooted || a.is_empty() {
        b.to_string()
    } else if a.ends_with(is_path_separator) || is_bare_drive(a) {
        format!("{a}{b}")
    } else {
        format!("{a}{PATH_SEPARATOR}{b}")
    }
}

/// `snd . splitDrive`. POSIX: without the leading slashes. Windows: without
/// a drive, UNC or device prefix and its root.
pub fn drop_drive(path: &str) -> &str {
    if cfg!(windows) {
        use std::path::{Component, Path};
        let mut comps = Path::new(path).components();
        let mut rest = comps.as_path();
        while let Some(Component::Prefix(_) | Component::RootDir) = comps.next() {
            rest = comps.as_path();
        }
        rest.to_str().unwrap_or(path)
    } else {
        path.trim_start_matches('/')
    }
}

/// `canonicalizePath` on Windows drops the `\\?\` prefix that the OS returns;
/// keeping it would also turn off `/` translation for every path later joined
/// onto the result.
pub fn strip_verbatim(path: &str) -> String {
    if cfg!(windows) {
        if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = path.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn posix() {
        assert_eq!(take_file_name("a/b.sh"), "b.sh");
        assert_eq!(take_file_name(r"a\b.sh"), r"a\b.sh");
        assert_eq!(take_directory("/a/b"), "/a");
        assert_eq!(take_directory("/"), "/");
        assert_eq!(take_directory("//"), "/");
        assert_eq!(take_directory("b"), ".");
        assert_eq!(combine("a", "b"), "a/b");
        assert_eq!(combine("a/", "b"), "a/b");
        assert_eq!(combine("a", "/b"), "/b");
        assert_eq!(combine("", "b"), "b");
        assert_eq!(drop_drive("//a/b"), "a/b");
        assert_eq!(strip_verbatim(r"\\?\C:\a"), r"\\?\C:\a");
    }

    #[cfg(windows)]
    #[test]
    fn windows() {
        assert_eq!(take_file_name(r"C:\a/b.sh"), "b.sh");
        assert_eq!(take_file_name("C:b.sh"), "b.sh");
        assert_eq!(take_directory(r"C:\a\b"), r"C:\a");
        assert_eq!(take_directory(r"C:\a"), r"C:\");
        assert_eq!(take_directory(r"C:\"), r"C:\");
        assert_eq!(take_directory("C:b"), "C:");
        assert_eq!(take_directory("b"), ".");
        assert_eq!(combine("a", "b"), r"a\b");
        assert_eq!(combine("a/", "b"), "a/b");
        assert_eq!(combine("C:", "b"), "C:b");
        assert_eq!(combine("a", r"D:\b"), r"D:\b");
        assert_eq!(drop_drive(r"C:\a\b"), r"a\b");
        assert_eq!(drop_drive(r"\\srv\share\a"), "a");
        assert_eq!(strip_verbatim(r"\\?\C:\a"), r"C:\a");
        assert_eq!(strip_verbatim(r"\\?\UNC\srv\s"), r"\\srv\s");
    }
}
