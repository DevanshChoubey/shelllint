// SPDX-License-Identifier: GPL-3.0-or-later
// Port of ShellCheck shellcheck.hs `ioInterface` and `inputFile` (e11f964);
// `decodeString` is shelllint_checker::decode_string.
//
// EditorConfig lookup lives in shelllint_checker::editorconfig; this file
// supplies its filesystem (`EditorConfigFs`) and merges the results.
// DIVERGENCE: caches live in `Mutex`es instead of `IORef`s so the interface is
// `Sync`.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use shelllint_checker::decode_string;
use shelllint_checker::editorconfig::{EditorConfigFs, get_editor_config, merge_configs};
use shelllint_checker::filepath::{
    PATH_SEPARATOR, combine, drop_drive, drop_file_name, is_path_separator, strip_verbatim,
};
use shelllint_core::interface::{ErrorMessage, SystemInterface};

use crate::options::Options;

pub struct IoInterface {
    external_sources: bool,
    source_paths: Vec<String>,
    source_functions: Vec<String>,
    source_from_path: bool,
    rcfile: Option<String>,
    inputs: Vec<String>,
    /// `--stdin-filename`: the name standard input is reported (and looked up) as.
    stdin_name: Option<String>,
    cache: Mutex<HashMap<String, String>>,
    config_cache: Mutex<(String, Option<(String, String)>)>,
}

pub fn io_interface(options: &Options, files: &[String]) -> IoInterface {
    IoInterface {
        external_sources: options.external_sources,
        source_paths: options.source_paths.clone(),
        source_functions: options.source_functions.clone(),
        source_from_path: options.source_from_path,
        rcfile: options.rcfile.clone(),
        inputs: files.iter().map(|f| normalize(f)).collect(),
        stdin_name: options.stdin_filename.clone(),
        cache: Mutex::new(HashMap::new()),
        config_cache: Mutex::new((String::new(), None)),
    }
}

fn normalize(x: &str) -> String {
    std::fs::canonicalize(x).map(|p| strip_verbatim(&p.to_string_lossy())).unwrap_or_else(|_| x.to_string())
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

pub fn file_exists(path: &str) -> bool {
    does_file_exist(path)
}

/// The text of the rc file that applies to `filename` (the `--rcfile` if
/// given), read without any warning output. Used for the formatter keys.
pub fn quiet_rc_text(rcfile: Option<&str>, filename: &str) -> Option<(String, String)> {
    let paths = match rcfile {
        Some(f) => vec![f.to_string()],
        None => IoInterface::config_paths(&dir_of(&normalize(filename))),
    };
    // Only regular files: reading a fifo or process substitution here would
    // consume what the real read later needs. (So `color=` in such an rc file
    // is not applied.)
    let is_regular = |p: &String| std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false);
    paths
        .iter()
        .find(|p| does_file_exist(p))
        .filter(|p| is_regular(p))
        .and_then(|p| input_file(p).ok().map(|r| (p.clone(), r.0)))
}

fn dir_of(path: &str) -> String {
    // System.FilePath takeDirectory
    match Path::new(path).parent() {
        Some(p) if p.as_os_str().is_empty() => ".".to_string(),
        Some(p) => p.to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

/// The base directory the config defaults hang off. POSIX: System.Directory
/// `getHomeDirectory` (`$HOME` when it is set, even to the empty string, else
/// the passwd entry's home directory). Windows: the roaming AppData folder
/// (`CSIDL_APPDATA`), which both `getAppUserDataDirectory` and the XdgConfig
/// fallback use there.
fn home_directory() -> Option<String> {
    if cfg!(windows) {
        std::env::var("APPDATA").ok().filter(|a| !a.is_empty())
    } else {
        #[allow(deprecated)]
        std::env::home_dir().map(|p| p.to_string_lossy().into_owned())
    }
}

/// System.Directory `getXdgDirectory XdgConfig suffix`: `$XDG_CONFIG_HOME`
/// when it is absolute, otherwise (unset, empty or relative, as the XDG Base
/// Directory spec says) `home </> ".config"` (Windows: AppData itself); then
/// `simplify (dir </> suffix)`. `None` when the fallback needs a home
/// directory and there is none.
fn xdg_config_path(home: Option<&str>, xdg: Option<&str>, suffix: &str) -> Option<String> {
    let dir = match xdg {
        Some(x) if Path::new(x).is_absolute() => x.to_string(),
        _ if cfg!(windows) => home?.to_string(),
        _ => combine(home?, ".config"),
    };
    Some(simplify_path(&combine(&dir, suffix)))
}

/// System.Directory's `simplify` (`normalise`): collapses repeated separators
/// and drops `.` components; `..` is kept. Windows also turns `/` into `\`.
fn simplify_path(p: &str) -> String {
    let parts: Vec<&str> = p.split(is_path_separator).filter(|c| !c.is_empty() && *c != ".").collect();
    let body = parts.join(&PATH_SEPARATOR.to_string());
    let trailing =
        if p.ends_with(is_path_separator) && !body.is_empty() { PATH_SEPARATOR.to_string() } else { String::new() };
    match (p.starts_with(is_path_separator), body.is_empty()) {
        (true, _) => format!("{}{}{}", PATH_SEPARATOR, body, trailing),
        (false, true) => ".".to_string(),
        (false, false) => format!("{}{}", body, trailing),
    }
}

impl IoInterface {
    /// `normalize`, except that the `--stdin-filename` path need not exist: its
    /// directory is resolved and the file name kept, as for a real file.
    fn normalize_input(&self, x: &str) -> String {
        if self.stdin_name.as_deref() == Some(x) && std::fs::canonicalize(x).is_err() {
            let p = Path::new(x);
            if let (Some(parent), Some(name)) = (p.parent(), p.file_name()) {
                let parent = if parent.as_os_str().is_empty() { Path::new(".") } else { parent };
                if let Ok(dir) = std::fs::canonicalize(parent) {
                    return combine(&strip_verbatim(&dir.to_string_lossy()), &name.to_string_lossy());
                }
            }
        }
        normalize(x)
    }

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

    /// `defaultPaths`: `getAppUserDataDirectory "shellcheckrc"` (POSIX `home
    /// ++ "/.shellcheckrc"`; Windows `appdata </> "shellcheckrc"`) and
    /// `getXdgDirectory XdgConfig "shellcheckrc"`. No home directory throws,
    /// and the handler gives `[]`.
    fn default_paths_from(home: Option<String>, xdg: Option<String>) -> Vec<String> {
        match home {
            Some(h) => {
                let x = xdg_config_path(Some(&h), xdg.as_deref(), "shellcheckrc").unwrap_or_default();
                let app = if cfg!(windows) { combine(&h, "shellcheckrc") } else { format!("{}/.shellcheckrc", h) };
                vec![app, x]
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
        let mut v = vec![combine(dir, ".shellcheckrc"), combine(dir, "shellcheckrc")];
        v.extend(rest);
        v
    }

    /// The .shellcheckrc (or --rcfile) that applies to `filename`, if any.
    pub fn rc_path(&self, filename: &str) -> Option<String> {
        self.get_rc_config(filename).map(|(path, _)| path)
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
                let path = self.normalize_input(filename);
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
        self.normalize_input(path)
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
    fn source_functions(&self) -> Vec<String> {
        self.source_functions.clone()
    }

    fn read_file(&self, rc_suggests_external: Option<bool>, file: &str) -> Result<String, ErrorMessage> {
        if let Some(x) = self.cache.lock().unwrap().get(file) {
            return Ok(x.clone());
        }
        if self.allowable(rc_suggests_external, file) {
            // `--stdin-filename`: the script named PATH is what arrives on stdin.
            let from = if self.stdin_name.as_deref() == Some(file) { "-" } else { file };
            match input_file(from) {
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
        let scriptdir = drop_file_name(current_script);
        let adjust = |s: &str| -> String {
            let mut parts = s.split(is_path_separator).filter(|p| !p.is_empty());
            if s.starts_with("SCRIPTDIR") && parts.next() == Some("SCRIPTDIR") {
                let rest: Vec<&str> = parts.collect();
                let mut p = PathBuf::from(scriptdir);
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
        let found = if Path::new(original).is_absolute() {
            find(drop_drive(original), original)
        } else {
            find(original, original)
        };
        // DIVERGENCE: `--source-from-path` (not in ShellCheck): a bare filename the
        // normal search did not find is looked up on `$PATH`, like bash's `source`.
        if self.source_from_path
            && found == original
            && !does_file_exist(original)
            && !original.contains(is_path_separator)
        {
            if let Some(path) = std::env::var_os("PATH") {
                for dir in std::env::split_paths(&path) {
                    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
                    let c = dir.join(original).to_string_lossy().into_owned();
                    if does_file_exist(&c) {
                        return c;
                    }
                }
            }
        }
        found
    }

    fn get_config(&self, filename: &str) -> Option<(String, String)> {
        // DIVERGENCE: `color=` and `wiki-link-count=` are applied by the CLI
        // (see rcstrict.rs), so the parser must not see them.
        let rc_result = self.get_rc_config(filename).map(|(p, c)| (p, crate::rcstrict::strip_extras(&c).0));
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
            "shelllint-sys-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&d).unwrap();
        PathBuf::from(normalize(&d.to_string_lossy()))
    }

    /// `base` joined with a `/`-separated relative path, the way the port
    /// joins it: native separators throughout.
    fn native(base: &Path, rel: &str) -> String {
        combine(&base.to_string_lossy(), &rel.replace('/', &PATH_SEPARATOR.to_string()))
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
        let s = |p: &str| native(&d, p);
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
        // An absolute name is also searched with its drive dropped, under each
        // search path (splitDrive). On Windows `/dup.sh` has no drive, so it is
        // not absolute, and `p1 </> "/dup.sh"` is `/dup.sh` itself.
        let want = if cfg!(windows) { "/dup.sh".to_string() } else { s("p1/dup.sh") };
        assert_eq!(sys.find_source(&cur, None, &[], "/dup.sh"), want);
        let abs = s("dup.sh");
        assert_eq!(sys.find_source(&cur, None, &[], &abs), abs); // missing: itself
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
        assert_eq!(path, native(&d, "shellcheckrc"));
        assert_eq!(c, "disable=SC1");
        // A dotted one closer wins; and .shellcheckrc beats shellcheckrc in one dir.
        write(&d.join("a/.shellcheckrc"), "disable=SC2");
        write(&d.join("a/shellcheckrc"), "disable=SC3");
        let sys = iface(false, &[], None, &[]);
        let (path, c) = sys.get_config(&script).unwrap();
        assert_eq!(path, native(&d, "a/.shellcheckrc"));
        assert_eq!(c, "disable=SC2");
        // Same-directory lookups are served from the cache even if the file vanishes.
        fs::remove_file(d.join("a/.shellcheckrc")).unwrap();
        assert_eq!(sys.get_config(&script).unwrap().1, "disable=SC2");
    }

    #[cfg(windows)]
    #[test]
    fn config_default_paths_appdata_and_xdg() {
        let v = IoInterface::default_paths_from(Some(r"C:\a".into()), None);
        assert_eq!(v, vec![r"C:\a\shellcheckrc", r"C:\a\shellcheckrc"]);
        let v = IoInterface::default_paths_from(Some(r"C:\a".into()), Some("D:/x//y".into()));
        assert_eq!(v, vec![r"C:\a\shellcheckrc", r"D:\x\y\shellcheckrc"]);
        // A drive-less XDG_CONFIG_HOME is not absolute on Windows.
        let v = IoInterface::default_paths_from(Some(r"C:\a".into()), Some("/x".into()));
        assert_eq!(v[1], r"C:\a\shellcheckrc");
        assert!(IoInterface::default_paths_from(None, None).is_empty());
    }

    #[test]
    fn config_walks_up_to_the_root_then_defaults() {
        let d = tmp();
        let home_rc = write(&d.join("home/.shellcheckrc"), "enable=all");
        let defaults = vec![home_rc.clone()];
        let paths = IoInterface::config_paths_with(&d.join("q").to_string_lossy(), &defaults);
        assert_eq!(paths.last().unwrap(), &home_rc);
        assert_eq!(paths[0], native(&d, "q/.shellcheckrc"));
        assert_eq!(paths[1], native(&d, "q/shellcheckrc"));
        let root = d.ancestors().last().unwrap().to_string_lossy().into_owned();
        assert!(paths.contains(&combine(&root, ".shellcheckrc")), "{:?}", paths);
        assert_eq!(IoInterface::read_config(&home_rc).unwrap().1, "enable=all");
    }

    #[cfg(unix)]
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
    }

    #[cfg(unix)] // uses /dev/null
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

    // The strerror texts are musl's (the oracle is a static musl build); only Linux uses that table.
    #[cfg(target_os = "linux")]
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
