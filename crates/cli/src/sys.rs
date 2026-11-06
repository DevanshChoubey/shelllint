// SPDX-License-Identifier: GPL-3.0-only
// Port of ShellCheck shellcheck.hs `ioInterface`, `inputFile`, `decodeString` (e11f964)
//
// DIVERGENCE: EditorConfig support (`getEditorConfig`, ShellCheck/EditorConfig.hs)
// is not ported; `get_config` only resolves .shellcheckrc / --rcfile.
// DIVERGENCE: caches live in `Mutex`es instead of `IORef`s so the interface is
// `Sync`.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use shellcheck_core::interface::{ErrorMessage, SystemInterface};

use crate::options::Options;

pub struct IoInterface {
    external_sources: bool,
    source_paths: Vec<String>,
    rcfile: Option<String>,
    inputs: Vec<String>,
    cache: Mutex<HashMap<String, String>>,
    config_cache: Mutex<(String, Option<(String, String)>)>,
}

pub fn io_interface(options: &Options, files: &[String]) -> IoInterface {
    IoInterface {
        external_sources: options.external_sources,
        source_paths: options.source_paths.clone(),
        rcfile: options.rcfile.clone(),
        inputs: files.iter().map(|f| normalize(f)).collect(),
        cache: Mutex::new(HashMap::new()),
        config_cache: Mutex::new((String::new(), None)),
    }
}

fn normalize(x: &str) -> String {
    std::fs::canonicalize(x)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| x.to_string())
}

/// Haskell `show` of the IOException `openBinaryFile` raises.
pub fn show_io_error(file: &str, e: &std::io::Error) -> String {
    show_io_error_in("openBinaryFile", file, e)
}

pub fn show_io_error_in(func: &str, file: &str, e: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    let desc = match e.kind() {
        NotFound => "does not exist (No such file or directory)".to_string(),
        PermissionDenied => "permission denied (Permission denied)".to_string(),
        _ if e.raw_os_error() == Some(21) => "inappropriate type (is a directory)".to_string(),
        _ => format!("failed ({})", e),
    };
    format!("{}: {}: {}", file, func, desc)
}

/// `inputFile`: returns (contents, shouldCache).
pub fn input_file(file: &str) -> Result<(String, bool), std::io::Error> {
    let mut bytes = vec![];
    let should_cache = if file == "-" {
        std::io::stdin().lock().read_to_end(&mut bytes)?;
        true
    } else {
        let mut f = std::fs::File::open(file)?;
        let meta = f.metadata()?;
        f.read_to_end(&mut bytes)?;
        // hIsSeekable: regular files can be re-read, pipes cannot.
        !meta.is_file()
    };
    Ok((decode_string(&bytes), should_cache))
}

/// Decode a char8 string into a utf8 string, with fallback on ISO-8859-1.
pub fn decode_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        if c < 0x80 {
            out.push(c as char);
            continue;
        }
        let num = c as u32;
        let (init, n) = if num >= 0xF8 {
            (None, 0)
        } else if num >= 0xF0 {
            (Some(num & 0x07), 3)
        } else if num >= 0xE0 {
            (Some(num & 0x0F), 2)
        } else if num >= 0xC0 {
            (Some(num & 0x1F), 1)
        } else {
            (None, 0)
        };
        let decoded = init.and_then(|x| construct(x, n, &bytes[i..]));
        match decoded {
            // DIVERGENCE: Haskell's `chr` accepts surrogates; Rust's `char` cannot
            // represent them, so such a sequence falls back to Latin-1 like an
            // invalid one (`c : decode rest`).
            Some((n, used)) if char::from_u32(n).is_some() => {
                out.push(char::from_u32(n).unwrap());
                i += used;
            }
            _ => out.push(c as char),
        }
    }
    out
}

fn construct(mut x: u32, n: usize, rest: &[u8]) -> Option<(u32, usize)> {
    for k in 0..n {
        let b = *rest.get(k)? as u32;
        if (0x80..=0xBF).contains(&b) {
            x = (x << 6) | (b & 0x3f);
        } else {
            return None;
        }
    }
    if x <= 0x10FFFF { Some((x, n)) } else { None }
}

fn dir_of(path: &str) -> String {
    // System.FilePath takeDirectory
    match Path::new(path).parent() {
        Some(p) if p.as_os_str().is_empty() => ".".to_string(),
        Some(p) => p.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

impl IoInterface {
    fn allowable(&self, rc_suggests_external: Option<bool>, x: &str) -> bool {
        if rc_suggests_external.unwrap_or(self.external_sources) {
            true
        } else {
            self.inputs.contains(&normalize(x))
        }
    }

    fn read_config(file: &str) -> Option<(String, String)> {
        if !Path::new(file).is_file() {
            return None;
        }
        match input_file(file) {
            Ok((contents, _)) => Some((file.to_string(), contents)),
            Err(e) => {
                eprintln!("{}: {}", file, show_io_error(file, &e));
                Some((file.to_string(), String::new()))
            }
        }
    }

    fn default_paths() -> Vec<String> {
        Self::default_paths_from(
            std::env::var("HOME").ok(),
            std::env::var("XDG_CONFIG_HOME").ok(),
        )
    }

    fn default_paths_from(home: Option<String>, xdg: Option<String>) -> Vec<String> {
        let home = home.filter(|h| !h.is_empty());
        let xdg = xdg
            .filter(|h| !h.is_empty())
            .or_else(|| home.as_ref().map(|h| format!("{}/.config", h)));
        match (home, xdg) {
            (Some(h), Some(x)) => vec![format!("{}/.shellcheckrc", h), format!("{}/shellcheckrc", x)],
            _ => vec![],
        }
    }

    fn config_paths(dir: &str) -> Vec<String> {
        Self::config_paths_with(dir, &Self::default_paths())
    }

    fn config_paths_with(dir: &str, defaults: &[String]) -> Vec<String> {
        let next = dir_of(dir);
        let rest = if next != dir {
            Self::config_paths_with(&next, defaults)
        } else {
            defaults.to_vec()
        };
        let join = |name: &str| {
            if dir.ends_with('/') { format!("{}{}", dir, name) } else { format!("{}/{}", dir, name) }
        };
        let mut v = vec![join(".shellcheckrc"), join("shellcheckrc")];
        v.extend(rest);
        v
    }

    fn get_rc_config(&self, filename: &str) -> Option<(String, String)> {
        let mut cache = self.config_cache.lock().unwrap();
        match &self.rcfile {
            Some(file) => {
                if cache.0 == "/" {
                    return cache.1.clone();
                }
                let result = Self::read_config(file);
                if result.is_none() {
                    eprintln!("Warning: unable to read --rcfile {}", file);
                }
                *cache = ("/".to_string(), result.clone());
                result
            }
            None => {
                let path = normalize(filename);
                let dir = dir_of(&path);
                if dir == cache.0 {
                    return cache.1.clone();
                }
                let result = Self::config_paths(&dir)
                    .iter()
                    .find_map(|p| Self::read_config(p));
                *cache = (dir, result.clone());
                result
            }
        }
    }
}

impl SystemInterface for IoInterface {
    fn read_file(&self, rc_suggests_external: Option<bool>, file: &str) -> Result<String, ErrorMessage> {
        if let Some(x) = self.cache.lock().unwrap().get(file) {
            return Ok(x.clone());
        }
        if self.allowable(rc_suggests_external, file) {
            match input_file(file) {
                Ok((contents, should_cache)) => {
                    if should_cache {
                        self.cache.lock().unwrap().insert(file.to_string(), contents.clone());
                    }
                    Ok(contents)
                }
                Err(e) => Err(show_io_error(file, &e)),
            }
        } else if rc_suggests_external == Some(false) {
            Err(format!(
                "{} was not specified as input, and external files were disabled via directive.",
                file
            ))
        } else {
            Err(format!("{} was not specified as input (see shellcheck -x).", file))
        }
    }

    fn find_source(
        &self,
        current_script: &str,
        rc_suggests_external: Option<bool>,
        source_path_annotation: &[String],
        original: &str,
    ) -> String {
        let scriptdir = {
            // dropFileName
            match current_script.rfind('/') {
                Some(i) => &current_script[..=i],
                None => "",
            }
            .to_string()
        };
        let adjust = |s: &str| -> String {
            let mut parts = s.split('/').filter(|p| !p.is_empty());
            if s.starts_with("SCRIPTDIR") && parts.next() == Some("SCRIPTDIR") {
                let rest: Vec<&str> = parts.collect();
                let mut p = PathBuf::from(&scriptdir);
                for r in rest {
                    p.push(r);
                }
                p.to_string_lossy().into_owned()
            } else {
                s.to_string()
            }
        };
        let find = |filename: &str, deflt: &str| -> String {
            let mut candidates = vec![adjust(filename)];
            for sp in self.source_paths.iter().chain(source_path_annotation.iter()) {
                candidates.push(Path::new(&adjust(sp)).join(filename).to_string_lossy().into_owned());
            }
            for c in candidates {
                if self.allowable(rc_suggests_external, &c) && Path::new(&c).is_file() {
                    return c;
                }
            }
            deflt.to_string()
        };
        if original.starts_with('/') {
            let relative = original.trim_start_matches('/');
            find(relative, original)
        } else {
            find(original, original)
        }
    }

    fn get_config(&self, filename: &str) -> Option<(String, String)> {
        self.get_rc_config(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::default_options;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "shellcheck-rust-sys-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    fn write(p: &Path, c: &str) -> String {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, c).unwrap();
        p.to_string_lossy().into_owned()
    }

    fn iface(ext: bool, sp: &[&str], rc: Option<&str>, files: &[&str]) -> IoInterface {
        let mut o = default_options();
        o.external_sources = ext;
        o.source_paths = sp.iter().map(|s| s.to_string()).collect();
        o.rcfile = rc.map(String::from);
        let files: Vec<String> = files.iter().map(|s| s.to_string()).collect();
        io_interface(&o, &files)
    }

    #[test]
    fn find_source_search_order() {
        let d = tmp();
        let s = |p: &str| d.join(p).to_string_lossy().into_owned();
        write(&d.join("lib.sh"), "");
        write(&d.join("p1/lib.sh"), "");
        write(&d.join("p2/only2.sh"), "");
        write(&d.join("ann/only3.sh"), "");
        let cur = s("main.sh");
        let sys = iface(true, &[&s("p1"), &s("p2")], None, &[]);
        let ann = vec![s("ann")];
        // Not in the cwd, so the -P paths are searched in order.
        assert_eq!(sys.find_source(&cur, None, &[], "lib.sh"), s("p1/lib.sh"));
        assert_eq!(sys.find_source(&cur, None, &[], "only2.sh"), s("p2/only2.sh"));
        assert_eq!(sys.find_source(&cur, None, &[], "missing.sh"), "missing.sh");
        // A file in the cwd wins over the -P paths.
        assert_eq!(sys.find_source(&cur, None, &[], "Cargo.toml"), "Cargo.toml");
        // Annotation paths come after the -P flags.
        assert_eq!(sys.find_source(&cur, None, &ann, "only3.sh"), s("ann/only3.sh"));
        // SCRIPTDIR expands to the directory of the current script.
        let sd = vec!["SCRIPTDIR/p1".to_string()];
        let sys2 = iface(true, &[], None, &[]);
        assert_eq!(sys2.find_source(&cur, None, &sd, "lib.sh"), format!("{}/p1/lib.sh", d.to_string_lossy()));
        assert_eq!(
            sys2.find_source(&cur, None, &[], "SCRIPTDIR/lib.sh"),
            format!("{}/lib.sh", d.to_string_lossy())
        );
        // Absolute path found as-is; missing absolute path falls back to itself.
        assert_eq!(sys2.find_source(&cur, None, &[], &s("lib.sh")), s("lib.sh"));
        assert_eq!(sys2.find_source(&cur, None, &[], "/nonexistent/zz.sh"), "/nonexistent/zz.sh");
    }

    #[test]
    fn find_source_respects_external_gate() {
        let d = tmp();
        let lib = write(&d.join("lib.sh"), "");
        let main = write(&d.join("main.sh"), "");
        let none = iface(false, &[], None, &[&main]);
        assert_eq!(none.find_source(&main, None, &[], &lib), lib); // default, not allowed
        let allowed = iface(false, &[], None, &[&main, &lib]);
        assert_eq!(allowed.find_source(&main, None, &[], &lib), lib);
        // rc/annotation Some(true) overrides, Some(false) forbids.
        let off = iface(true, &[], None, &[]);
        let sp = vec![d.to_string_lossy().into_owned()];
        assert_eq!(off.find_source(&main, Some(false), &sp, "lib.sh"), "lib.sh");
        assert_eq!(off.find_source(&main, Some(true), &sp, "lib.sh"), lib);
    }

    #[test]
    fn config_walks_up_and_prefers_dotfile() {
        let d = tmp();
        write(&d.join("shellcheckrc"), "disable=SC1");
        let sub = d.join("a/b");
        let script = write(&sub.join("x.sh"), "");
        let sys = iface(false, &[], None, &[]);
        // Only the undotted file exists up the tree.
        let (path, c) = sys.get_config(&script).unwrap();
        assert_eq!(path, format!("{}/shellcheckrc", d.to_string_lossy()));
        assert_eq!(c, "disable=SC1");
        // A dotted one closer wins; and .shellcheckrc beats shellcheckrc in one dir.
        write(&d.join("a/.shellcheckrc"), "disable=SC2");
        write(&d.join("a/shellcheckrc"), "disable=SC3");
        let sys = iface(false, &[], None, &[]);
        let (path, c) = sys.get_config(&script).unwrap();
        assert_eq!(path, format!("{}/a/.shellcheckrc", d.to_string_lossy()));
        assert_eq!(c, "disable=SC2");
        // Same-directory lookups are served from the cache even if the file vanishes.
        fs::remove_file(d.join("a/.shellcheckrc")).unwrap();
        assert_eq!(sys.get_config(&script).unwrap().1, "disable=SC2");
    }

    #[test]
    fn config_default_paths_home_and_xdg() {
        let v = IoInterface::default_paths_from(Some("/h".into()), None);
        assert_eq!(v, vec!["/h/.shellcheckrc", "/h/.config/shellcheckrc"]);
        let v = IoInterface::default_paths_from(Some("/h".into()), Some("/x".into()));
        assert_eq!(v, vec!["/h/.shellcheckrc", "/x/shellcheckrc"]);
        assert!(IoInterface::default_paths_from(None, None).is_empty());
        // They are searched after every directory up to the root.
        let d = tmp();
        let home_rc = write(&d.join("home/.shellcheckrc"), "enable=all");
        let defaults = vec![home_rc.clone()];
        let paths = IoInterface::config_paths_with(&d.join("q").to_string_lossy(), &defaults);
        assert_eq!(paths.last().unwrap(), &home_rc);
        assert_eq!(paths[0], format!("{}/q/.shellcheckrc", d.to_string_lossy()));
        assert_eq!(paths[1], format!("{}/q/shellcheckrc", d.to_string_lossy()));
        assert!(paths.contains(&"/.shellcheckrc".to_string()));
        assert_eq!(IoInterface::read_config(&home_rc).unwrap().1, "enable=all");
    }

    #[test]
    fn rcfile_is_cached_with_slash_sentinel() {
        let d = tmp();
        let rc = write(&d.join("my.rc"), "disable=SC9");
        write(&d.join("shellcheckrc"), "ignored");
        let sys = iface(false, &[], Some(&rc), &[]);
        let other = write(&d.join("x/y.sh"), "");
        assert_eq!(sys.get_config(&other), Some((rc.clone(), "disable=SC9".into())));
        assert_eq!(sys.config_cache.lock().unwrap().0, "/");
        // Cached: removing the file or asking about other scripts changes nothing.
        fs::remove_file(&rc).unwrap();
        assert_eq!(sys.get_config("/elsewhere/z.sh"), Some((rc.clone(), "disable=SC9".into())));
        // A missing rcfile caches None.
        let sys = iface(false, &[], Some("/nonexistent/rc"), &[]);
        assert_eq!(sys.get_config(&other), None);
        assert_eq!(sys.config_cache.lock().unwrap().0, "/");
        assert_eq!(sys.get_config(&other), None);
    }

    #[test]
    fn read_file_external_gating() {
        let d = tmp();
        let main = write(&d.join("main.sh"), "echo main");
        let lib = write(&d.join("lib.sh"), "echo lib");
        let sys = iface(false, &[], None, &[&main]);
        assert_eq!(sys.read_file(None, &main), Ok("echo main".into()));
        assert_eq!(
            sys.read_file(None, &lib),
            Err(format!("{} was not specified as input (see shellcheck -x).", lib))
        );
        assert_eq!(
            sys.read_file(Some(false), &lib),
            Err(format!(
                "{} was not specified as input, and external files were disabled via directive.",
                lib
            ))
        );
        assert_eq!(sys.read_file(Some(true), &lib), Ok("echo lib".into()));
        let x = iface(true, &[], None, &[]);
        assert_eq!(x.read_file(None, &lib), Ok("echo lib".into()));
        assert_eq!(x.read_file(Some(false), &lib).unwrap_err().contains("disabled via directive"), true);
        let e = x.read_file(None, "/nonexistent/q.sh").unwrap_err();
        assert_eq!(e, "/nonexistent/q.sh: openBinaryFile: does not exist (No such file or directory)");
    }

    #[test]
    fn decode_string_utf8_and_latin1_fallback() {
        assert_eq!(decode_string(b"abc"), "abc");
        assert_eq!(decode_string("h\u{e9}\u{20ac}\u{1f600}".as_bytes()), "h\u{e9}\u{20ac}\u{1f600}");
        // Invalid UTF-8 bytes become the Latin-1 characters of the same value.
        assert_eq!(decode_string(&[0x68, 0xe9, 0x21]), "h\u{e9}!");
        assert_eq!(decode_string(&[0xc3]), "\u{c3}"); // truncated sequence
        assert_eq!(decode_string(&[0xc3, 0x41]), "\u{c3}A"); // bad continuation
        assert_eq!(decode_string(&[0x80]), "\u{80}"); // stray continuation
        assert_eq!(decode_string(&[0xf8, 0x80]), "\u{f8}\u{80}"); // 5-byte lead
        // Surrogate (ED A0 80): not representable, falls back byte by byte.
        assert_eq!(decode_string(&[0xed, 0xa0, 0x80]), "\u{ed}\u{a0}\u{80}");
        // Beyond U+10FFFF (F4 90 80 80) is rejected.
        assert_eq!(decode_string(&[0xf4, 0x90, 0x80, 0x80]), "\u{f4}\u{90}\u{80}\u{80}");
    }
}
