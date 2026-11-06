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
            Some((n, used)) => {
                out.push(char::from_u32(n).unwrap_or(c as char));
                i += used;
            }
            None => out.push(c as char),
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

fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace())
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
        let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
        let xdg = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|h| !h.is_empty())
            .or_else(|| home.as_ref().map(|h| format!("{}/.config", h)));
        match (home, xdg) {
            (Some(h), Some(x)) => vec![format!("{}/.shellcheckrc", h), format!("{}/shellcheckrc", x)],
            _ => vec![],
        }
    }

    fn config_paths(dir: &str) -> Vec<String> {
        let next = dir_of(dir);
        let rest = if next != dir {
            Self::config_paths(&next)
        } else {
            Self::default_paths()
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

pub fn trim_line(s: &str) -> &str {
    trim(s)
}
