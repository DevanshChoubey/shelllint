// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: not in upstream ShellCheck. `source-path=RCDIR` in a
// .shellcheckrc names the directory holding that rc file, the way
// `SCRIPTDIR` names the script's directory.

use shelllint_core::ast::Annotation;

/// Replaces a leading `RCDIR` path component of each `source-path` annotation
/// with the directory of the rc file `rc_file`. Anything else, including a
/// path that merely starts with `RCDIR` (such as `RCDIRS/x`), is unchanged.
pub(crate) fn expand_rcdir(rc_file: &str, annotations: Vec<Annotation>) -> Vec<Annotation> {
    annotations
        .into_iter()
        .map(|a| match a {
            Annotation::SourcePath(p) => Annotation::SourcePath(expand_one(rc_file, p)),
            other => other,
        })
        .collect()
}

fn expand_one(rc_file: &str, path: String) -> String {
    let Some(rest) = path.strip_prefix("RCDIR") else { return path };
    if !(rest.is_empty() || rest.starts_with('/')) {
        return path;
    }
    let dir = match rc_file.rfind('/') {
        Some(0) => "/",
        Some(i) => &rc_file[..i],
        None => ".",
    };
    format!("{}{}", dir.trim_end_matches('/'), rest)
}

#[cfg(test)]
mod tests {
    use super::expand_one;

    #[test]
    fn expands_only_a_leading_component() {
        assert_eq!(expand_one("/a/b/.shellcheckrc", "RCDIR".into()), "/a/b");
        assert_eq!(expand_one("/a/b/.shellcheckrc", "RCDIR/sub/dir".into()), "/a/b/sub/dir");
        assert_eq!(expand_one("/.shellcheckrc", "RCDIR/x".into()), "/x");
        assert_eq!(expand_one(".shellcheckrc", "RCDIR/x".into()), "./x");
        assert_eq!(expand_one("/a/.shellcheckrc", "RCDIRS/x".into()), "RCDIRS/x");
        assert_eq!(expand_one("/a/.shellcheckrc", "x/RCDIR".into()), "x/RCDIR");
    }
}
