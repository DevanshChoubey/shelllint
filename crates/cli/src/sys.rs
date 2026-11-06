// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck shellcheck.hs `ioInterface` and `inputFile` (e11f964);
// `decodeString` is shellcheck_checker::decode_string.
//
// EditorConfig lookup lives in shellcheck_checker::editorconfig; this file
// supplies its filesystem (`EditorConfigFs`) and merges the results.
// DIVERGENCE: caches live in `Mutex`es instead of `IORef`s so the interface is
// `Sync`.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use shellcheck_checker::decode_string;
use shellcheck_checker::editorconfig::{EditorConfigFs, get_editor_config, merge_configs};
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
    std::fs::canonicalize(x).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| x.to_string())
}

/// Haskell `show` of the IOException `openBinaryFile` raises.
pub fn show_io_error(file: &str, e: &std::io::Error) -> String {
    show_io_error_in("openBinaryFile", file, e)
}

pub fn show_io_error_in(func: &str, file: &str, e: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    if let Some(closed) = e.get_ref().and_then(|x| x.downcast_ref::<StdinClosed>()) {
        return closed.to_string();
    }
    if let Some(r) = e.get_ref().and_then(|x| x.downcast_ref::<ReadFailed>()) {
        return show_io_error_in("hGetContents", file, &r.0);
    }
    let desc = match e.kind() {
        NotFound => "does not exist (No such file or directory)".to_string(),
        PermissionDenied => "permission denied (Permission denied)".to_string(),
        _ if e.raw_os_error() == Some(21) => "inappropriate type (is a directory)".to_string(),
        _ => match e.raw_os_error() {
            Some(n) => format!("{} ({})", ghc_errno_type(n), strerror(e)),
            None => format!("failed ({})", e),
        },
    };
    format!("{}: {}: {}", file, func, desc)
}

/// A failure while reading an already opened file: GHC reports it from
/// `hGetContents` rather than `openBinaryFile`.
#[derive(Debug)]
pub struct ReadFailed(pub std::io::Error);

impl std::fmt::Display for ReadFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ReadFailed {}

/// musl `strerror` text for the errnos `ghc_errno_type` classifies. The
/// reference binary is statically linked against musl, whose wording differs
/// from glibc's for several of them (e.g. ELOOP, ENAMETOOLONG).
#[cfg(target_os = "linux")]
fn musl_strerror(n: i32) -> Option<&'static str> {
    use libc::*;
    Some(match n {
        E2BIG => "Argument list too long",
        EACCES => "Permission denied",
        EADDRINUSE => "Address in use",
        EADDRNOTAVAIL => "Address not available",
        EAFNOSUPPORT => "Address family not supported by protocol",
        EAGAIN => "Resource temporarily unavailable",
        EALREADY => "Operation already in progress",
        EBADF => "Bad file descriptor",
        EBUSY => "Resource busy",
        ECHILD => "No child process",
        ECONNREFUSED => "Connection refused",
        ECONNRESET => "Connection reset by peer",
        EDEADLK => "Resource deadlock would occur",
        EDESTADDRREQ => "Destination address required",
        EDOM => "Domain error",
        EDQUOT => "Quota exceeded",
        EEXIST => "File exists",
        EFBIG => "File too large",
        EHOSTUNREACH => "Host is unreachable",
        EIDRM => "Identifier removed",
        EILSEQ => "Illegal byte sequence",
        EINPROGRESS => "Operation in progress",
        EINTR => "Interrupted system call",
        EINVAL => "Invalid argument",
        EIO => "I/O error",
        EISCONN => "Socket is connected",
        EISDIR => "Is a directory",
        ELOOP => "Symbolic link loop",
        EMFILE | ENFILE => "No file descriptors available",
        EMLINK => "Too many links",
        EMSGSIZE => "Message too long",
        ENAMETOOLONG => "Filename too long",
        ENETDOWN => "Network is down",
        ENETRESET => "Connection reset by network",
        ENETUNREACH => "Network unreachable",
        ENOBUFS => "No buffer space available",
        ENODATA => "No data available",
        ENODEV => "No such device",
        ENOENT => "No such file or directory",
        ENOEXEC => "Exec format error",
        ENOLCK => "No locks available",
        ENOLINK => "Link has been severed",
        ENOMEM => "Out of memory",
        ENOMSG => "No message of desired type",
        ENONET => "Machine is not on the network",
        ENOPROTOOPT => "Protocol not available",
        ENOSPC => "No space left on device",
        ENOSR => "Out of streams resources",
        ENOSTR => "Device not a stream",
        ENOSYS => "Function not implemented",
        ENOTBLK => "Block device required",
        ENOTCONN => "Socket not connected",
        ENOTDIR => "Not a directory",
        ENOTEMPTY => "Directory not empty",
        ENOTSOCK => "Socket operation on non-socket",
        ENOTTY => "Not a tty",
        ENXIO => "No such device or address",
        EOPNOTSUPP => "Not supported",
        EPERM => "Operation not permitted",
        EPIPE => "Broken pipe",
        EPROTO => "Protocol error",
        EPROTONOSUPPORT => "Protocol not supported",
        EPROTOTYPE => "Protocol wrong type for socket",
        ERANGE => "Result not representable",
        EROFS => "Read-only file system",
        ESPIPE => "Invalid seek",
        ESRCH => "No such process",
        ESTALE => "Stale file handle",
        ETIME => "Timer expired",
        ETIMEDOUT => "Operation timed out",
        ETXTBSY => "Text file busy",
        EXDEV => "Cross-device link",
        _ => return None,
    })
}

#[cfg(not(target_os = "linux"))]
fn musl_strerror(_n: i32) -> Option<&'static str> {
    None
}

/// C `strerror` text of an OS error: Rust's `Display` is `"<strerror> (os error N)"`.
fn strerror(e: &std::io::Error) -> String {
    if let Some(m) = e.raw_os_error().and_then(musl_strerror) {
        return m.to_string();
    }
    let t = e.to_string();
    match t.rfind(" (os error ") {
        Some(i) => t[..i].to_string(),
        None => t,
    }
}

/// GHC's `IOErrorType` `show` text for an errno, per `errnoToIOError`
/// (Foreign.C.Error). Errnos it does not list are `OtherError` ("failed").
#[cfg(target_os = "linux")]
fn ghc_errno_type(n: i32) -> &'static str {
    use libc::*;
    match n {
        E2BIG | EAGAIN | EMFILE | EMLINK | EMSGSIZE | ENFILE | ENOBUFS | ENOLCK | ENOMEM | ENOSPC | ENOSR => {
            "resource exhausted"
        }
        EACCES | EDQUOT | EFBIG | EPERM | EROFS => "permission denied",
        EADDRINUSE | EBUSY | EDEADLK | ETXTBSY => "resource busy",
        EADDRNOTAVAIL | EAFNOSUPPORT | ENODEV | ENOPROTOOPT | ENOSYS | EOPNOTSUPP | ERANGE | ESPIPE | EXDEV => {
            "unsupported operation"
        }
        EALREADY | EEXIST | EINPROGRESS | EISCONN => "already exists",
        EBADF | EDESTADDRREQ | EDOM | EILSEQ | EINVAL | ELOOP | ENAMETOOLONG | ENOEXEC | ENOSTR | ENOTBLK
        | ENOTCONN | ENOTSOCK => "invalid argument",
        ECHILD | ECONNREFUSED | EHOSTUNREACH | ENETUNREACH | ENODATA | ENOENT | ENOMSG | ENONET | ENXIO | ESRCH => {
            "does not exist"
        }
        ECONNRESET | EIDRM | ENETDOWN | ENETRESET | ENOLINK | EPIPE | ESTALE => "resource vanished",
        EISDIR | ENOTDIR => "inappropriate type",
        EINTR => "interrupted",
        EIO => "hardware fault",
        ENOTEMPTY => "unsatisfied constraints",
        ENOTTY => "illegal operation",
        EPROTO | EPROTONOSUPPORT | EPROTOTYPE => "protocol error",
        ETIME | ETIMEDOUT => "timeout",
        _ => "failed",
    }
}

#[cfg(not(target_os = "linux"))]
fn ghc_errno_type(_n: i32) -> &'static str {
    "failed"
}

/// Whether stdin has been read to the end. Haskell's `getContents` and
/// `hGetContents stdin` close it (`getContents` semi-closes it until the
/// lazy string is forced), so stdin can only be read once.
static STDIN_CONSUMED: AtomicBool = AtomicBool::new(false);

/// The IOException a read of an already consumed stdin raises:
/// `<stdin>: hGetContents: illegal operation (handle is closed)`, or
/// `semi-closed` while a `getContents` string is still unforced.
#[derive(Debug)]
pub struct StdinClosed {
    pub semi: bool,
}

impl std::fmt::Display for StdinClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.semi { "semi-closed" } else { "closed" };
        write!(f, "<stdin>: hGetContents: illegal operation (handle is {})", state)
    }
}

impl std::error::Error for StdinClosed {}

/// Read all of stdin once; a later call fails like Haskell's
/// `hGetContents` on a closed (`semi`: semi-closed) handle.
pub fn read_stdin(semi: bool) -> Result<Vec<u8>, std::io::Error> {
    if STDIN_CONSUMED.swap(true, Ordering::SeqCst) {
        return Err(std::io::Error::other(StdinClosed { semi }));
    }
    let mut bytes = vec![];
    std::io::stdin().lock().read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// `inputFile`: returns (contents, shouldCache).
pub fn input_file(file: &str) -> Result<(String, bool), std::io::Error> {
    let mut bytes = vec![];
    let should_cache = if file == "-" {
        bytes = read_stdin(false)?;
        true
    } else {
        let mut f = std::fs::File::open(file)?;
        let meta = f.metadata()?;
        f.read_to_end(&mut bytes).map_err(|e| match e.raw_os_error() {
            Some(n) if n != 21 => std::io::Error::new(e.kind(), ReadFailed(e)),
            _ => e,
        })?;
        // hIsSeekable: regular files can be re-read, pipes cannot.
        !meta.is_file()
    };
    Ok((decode_string(&bytes), should_cache))
}

/// System.Directory `doesFileExist`: the path exists (following symlinks)
/// and is not a directory, so fifos, devices and `/proc/self/fd/N` (process
/// substitution) count.
fn does_file_exist(path: &str) -> bool {
    std::fs::metadata(path).map(|m| !m.is_dir()).unwrap_or(false)
}

fn dir_of(path: &str) -> String {
    // System.FilePath takeDirectory
    match Path::new(path).parent() {
        Some(p) if p.as_os_str().is_empty() => ".".to_string(),
        Some(p) => p.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

/// System.Directory `getHomeDirectory` (POSIX): `$HOME` when it is set, even
/// to the empty string, else the passwd entry's home directory.
fn home_directory() -> Option<String> {
    #[allow(deprecated)]
    std::env::home_dir().map(|p| p.to_string_lossy().into_owned())
}

/// System.Directory `getXdgDirectory XdgConfig suffix`: `$XDG_CONFIG_HOME`
/// when it is absolute, otherwise (unset, empty or relative, as the XDG Base
/// Directory spec says) `home </> ".config"`; then `simplify (dir </> suffix)`.
/// `None` when the fallback needs a home directory and there is none.
fn xdg_config_path(home: Option<&str>, xdg: Option<&str>, suffix: &str) -> Option<String> {
    let dir = match xdg {
        Some(x) if x.starts_with('/') => x.to_string(),
        _ => join_path(home?, ".config"),
    };
    Some(simplify_path(&join_path(&dir, suffix)))
}

/// System.FilePath `</>` (POSIX) for a relative right-hand side.
fn join_path(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{}{}", a, b)
    } else {
        format!("{}/{}", a, b)
    }
}

/// System.Directory's `simplify` (POSIX `normalise`): collapses repeated
/// slashes and drops `.` components; `..` is kept.
fn simplify_path(p: &str) -> String {
    let parts: Vec<&str> = p.split('/').filter(|c| !c.is_empty() && *c != ".").collect();
    let body = parts.join("/");
    let trailing = if p.ends_with('/') && !body.is_empty() { "/" } else { "" };
    match (p.starts_with('/'), body.is_empty()) {
        (true, _) => format!("/{}{}", body, trailing),
        (false, true) => ".".to_string(),
        (false, false) => format!("{}{}", body, trailing),
    }
}

impl IoInterface {
    fn allowable(&self, rc_suggests_external: Option<bool>, x: &str) -> bool {
        if rc_suggests_external.unwrap_or(self.external_sources) { true } else { self.inputs.contains(&normalize(x)) }
    }

    fn read_config(file: &str) -> Option<(String, String)> {
        if !does_file_exist(file) {
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
        Self::default_paths_from(home_directory(), std::env::var("XDG_CONFIG_HOME").ok())
    }

    /// `defaultPaths`: `getAppUserDataDirectory "shellcheckrc"` (`home ++
    /// "/.shellcheckrc"`) and `getXdgDirectory XdgConfig "shellcheckrc"`. No
    /// home directory throws, and the handler gives `[]`.
    fn default_paths_from(home: Option<String>, xdg: Option<String>) -> Vec<String> {
        match home {
            Some(h) => {
                let x = xdg_config_path(Some(&h), xdg.as_deref(), "shellcheckrc").unwrap_or_default();
                vec![format!("{}/.shellcheckrc", h), x]
            }
            None => vec![],
        }
    }

    fn config_paths(dir: &str) -> Vec<String> {
        Self::config_paths_with(dir, &Self::default_paths())
    }

    fn config_paths_with(dir: &str, defaults: &[String]) -> Vec<String> {
        let next = dir_of(dir);
        let rest = if next != dir { Self::config_paths_with(&next, defaults) } else { defaults.to_vec() };
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
                let result = Self::config_paths(&dir).iter().find_map(|p| Self::read_config(p));
                *cache = (dir, result.clone());
                result
            }
        }
    }
}

impl EditorConfigFs for IoInterface {
    fn normalize(&self, path: &str) -> String {
        normalize(path)
    }

    fn read_config(&self, file: &str) -> Option<(String, String)> {
        Self::read_config(file)
    }

    /// `getXdgDirectory XdgConfig "editorconfig.ini"`
    fn global_editor_config_path(&self) -> Option<String> {
        let xdg = std::env::var("XDG_CONFIG_HOME").ok();
        xdg_config_path(home_directory().as_deref(), xdg.as_deref(), "editorconfig.ini")
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
            Err(format!("{} was not specified as input, and external files were disabled via directive.", file))
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
                if self.allowable(rc_suggests_external, &c) && does_file_exist(&c) {
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
        let rc_result = self.get_rc_config(filename);
        let ec_result = get_editor_config(self, filename);
        merge_configs(rc_result, ec_result)
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
        let ds = d.to_string_lossy().into_owned();
        let s = |p: &str| format!("{}/{}", ds, p);
        // The same name in both -P dirs, so the order is observable.
        write(&d.join("p1/dup.sh"), "");
        write(&d.join("p2/dup.sh"), "");
        write(&d.join("p2/only2.sh"), "");
        write(&d.join("ann/only3.sh"), "");
        write(&d.join("ann/dup.sh"), "");
        let cur = s("main.sh");
        let sys = iface(true, &[&s("p1"), &s("p2")], None, &[]);
        // -P order: p1 before p2, then annotation paths.
        assert_eq!(sys.find_source(&cur, None, &[], "dup.sh"), s("p1/dup.sh"));
        let sys_rev = iface(true, &[&s("p2"), &s("p1")], None, &[]);
        assert_eq!(sys_rev.find_source(&cur, None, &[], "dup.sh"), s("p2/dup.sh"));
        assert_eq!(sys.find_source(&cur, None, &[], "only2.sh"), s("p2/only2.sh"));
        let ann = vec![s("ann")];
        assert_eq!(sys.find_source(&cur, None, &ann, "dup.sh"), s("p1/dup.sh"));
        assert_eq!(sys.find_source(&cur, None, &ann, "only3.sh"), s("ann/only3.sh"));
        assert_eq!(sys.find_source(&cur, None, &[], "missing.sh"), "missing.sh");
        // cwd precedence: a name that exists relative to the cwd (the crate dir
        // under `cargo test`) wins over every search path.
        write(&d.join("p1/Cargo.toml"), "");
        assert!(Path::new("Cargo.toml").is_file());
        assert_eq!(sys.find_source(&cur, None, &[], "Cargo.toml"), "Cargo.toml");
        // SCRIPTDIR expands to the directory of the current script.
        let sd = vec!["SCRIPTDIR/p2".to_string()];
        let sys2 = iface(true, &[], None, &[]);
        assert_eq!(sys2.find_source(&cur, None, &sd, "dup.sh"), s("p2/dup.sh"));
        assert_eq!(sys2.find_source(&cur, None, &[], "SCRIPTDIR/p1/dup.sh"), s("p1/dup.sh"));
        // Absolute path: found as-is; a missing one falls back to itself.
        assert_eq!(sys2.find_source(&cur, None, &[], &s("p2/dup.sh")), s("p2/dup.sh"));
        assert_eq!(sys2.find_source(&cur, None, &[], "/nonexistent/zz.sh"), "/nonexistent/zz.sh");
        // An absolute name is also searched with its leading slash dropped, under
        // each search path (splitDrive).
        assert_eq!(sys.find_source(&cur, None, &[], "/dup.sh"), s("p1/dup.sh"));
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
        assert!(IoInterface::default_paths_from(None, Some("/x".into())).is_empty());
        // An empty or relative XDG_CONFIG_HOME is ignored (getXdgDirectory).
        for x in ["", "xdg", "./xdg", "../w/xdg", "xdg/"] {
            let v = IoInterface::default_paths_from(Some("/h".into()), Some(x.into()));
            assert_eq!(v, vec!["/h/.shellcheckrc", "/h/.config/shellcheckrc"], "XDG_CONFIG_HOME={:?}", x);
        }
        // Checked against the oracle with strace: the XDG path is simplified,
        // the home one is a plain concatenation, and an empty HOME is used as is.
        let v = IoInterface::default_paths_from(Some("/hh/".into()), Some("//x/./y//".into()));
        assert_eq!(v, vec!["/hh//.shellcheckrc", "/x/y/shellcheckrc"]);
        let v = IoInterface::default_paths_from(Some("".into()), Some("rel".into()));
        assert_eq!(v, vec!["/.shellcheckrc", ".config/shellcheckrc"]);
        assert_eq!(xdg_config_path(None, Some("/x"), "editorconfig.ini").as_deref(), Some("/x/editorconfig.ini"));
        assert_eq!(xdg_config_path(None, Some("x"), "editorconfig.ini"), None);
        assert_eq!(
            xdg_config_path(Some("/h/"), Some("x"), "editorconfig.ini").as_deref(),
            Some("/h/.config/editorconfig.ini")
        );
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
    fn does_file_exist_accepts_non_regular_files() {
        let d = tmp();
        let f = write(&d.join("f"), "");
        assert!(does_file_exist(&f));
        assert!(does_file_exist("/dev/null"));
        assert!(!does_file_exist(&d.to_string_lossy()));
        assert!(!does_file_exist("/nonexistent/x"));
        // /dev/null as --rcfile is read (empty), with no warning.
        let sys = iface(false, &[], Some("/dev/null"), &[]);
        assert_eq!(sys.get_config(&f), Some(("/dev/null".into(), String::new())));
        // find_source accepts a device too.
        let sys = iface(true, &[], None, &[]);
        assert_eq!(sys.find_source(&f, None, &["/dev".into()], "null"), "/dev/null");
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
        assert_eq!(sys.read_file(None, &lib), Err(format!("{} was not specified as input (see shellcheck -x).", lib)));
        assert_eq!(
            sys.read_file(Some(false), &lib),
            Err(format!("{} was not specified as input, and external files were disabled via directive.", lib))
        );
        assert_eq!(sys.read_file(Some(true), &lib), Ok("echo lib".into()));
        let x = iface(true, &[], None, &[]);
        assert_eq!(x.read_file(None, &lib), Ok("echo lib".into()));
        assert!(x.read_file(Some(false), &lib).unwrap_err().contains("disabled via directive"));
        let e = x.read_file(None, "/nonexistent/q.sh").unwrap_err();
        assert_eq!(e, "/nonexistent/q.sh: openBinaryFile: does not exist (No such file or directory)");
    }

    #[test]
    fn ghc_error_text_for_other_errnos() {
        let show = |n: i32| show_io_error("f", &std::io::Error::from_raw_os_error(n));
        assert_eq!(show(libc::ENOTDIR), "f: openBinaryFile: inappropriate type (Not a directory)");
        assert_eq!(show(libc::ENAMETOOLONG), "f: openBinaryFile: invalid argument (Filename too long)");
        assert_eq!(show(libc::ELOOP), "f: openBinaryFile: invalid argument (Symbolic link loop)");
        assert_eq!(show(libc::EIO), "f: openBinaryFile: hardware fault (I/O error)");
        assert_eq!(show(libc::EMFILE), "f: openBinaryFile: resource exhausted (No file descriptors available)");
        assert_eq!(show(libc::EROFS), "f: openBinaryFile: permission denied (Read-only file system)");
        assert_eq!(show(libc::ETXTBSY), "f: openBinaryFile: resource busy (Text file busy)");
        assert_eq!(show(libc::EINVAL), "f: openBinaryFile: invalid argument (Invalid argument)");
        // Not in errnoToIOError's table: OtherError, "failed".
        assert!(show(libc::EFAULT).starts_with("f: openBinaryFile: failed ("));
        let read = std::io::Error::other(ReadFailed(std::io::Error::from_raw_os_error(libc::EIO)));
        assert_eq!(show_io_error("f", &read), "f: hGetContents: hardware fault (I/O error)");
    }
}
