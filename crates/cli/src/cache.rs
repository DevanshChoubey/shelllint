// SPDX-License-Identifier: GPL-3.0-or-later
// `--cache-dir=PATH` (an extension beyond ShellCheck, koalaman/shellcheck#2939):
// reuse the result of checking a file when nothing that could change it has.
//
// A cached entry holds the `CheckResult` plus a log of every question the
// checker asked the `SystemInterface` while producing it (which rc/editorconfig
// applied, where each `source` resolved, the bytes of each sourced file). A
// lookup replays that log against the live interface and compares the answers
// exactly; any difference, any unreadable or malformed entry, is a miss. The
// checker is deterministic given the script, the options and those answers, so
// a hit is the same as a fresh run. The entry name is a hash of the version,
// the script bytes and every option that affects results.
//
// DIVERGENCE: the whole feature is an extension; the hash is an in-tree
// 128-bit FNV-1a (non-cryptographic, which is fine for a local cache whose
// entries are also verified against the full key stored inside them).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use shelllint_checker::check_script_owned;
use shelllint_core::data::SHELLCHECK_VERSION;
use shelllint_core::interface::{
    CheckResult, CheckSpec, Comment, ErrorMessage, Fix, InsertionPoint, Position, PositionedComment, Replacement,
    Severity, Shell, SystemInterface,
};

use crate::options::Options;

const MAGIC: &[u8] = b"SHELLLINT-CACHE-1\n";

/// 128-bit FNV-1a.
fn hash128(bytes: &[u8]) -> u128 {
    const OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013B;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u128;
        h = h.wrapping_mul(PRIME);
    }
    h
}

// ---- encoding --------------------------------------------------------------

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, n: u8) {
        self.0.push(n);
    }
    fn u64(&mut self, n: u64) {
        self.0.extend_from_slice(&n.to_le_bytes());
    }
    fn i64(&mut self, n: i64) {
        self.0.extend_from_slice(&n.to_le_bytes());
    }
    fn u128(&mut self, n: u128) {
        self.0.extend_from_slice(&n.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn opt_bool(&mut self, b: Option<bool>) {
        self.u8(match b {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    fn i64(&mut self) -> Option<i64> {
        self.take(8).map(|b| i64::from_le_bytes(b.try_into().unwrap()))
    }
    fn u128(&mut self) -> Option<u128> {
        self.take(16).map(|b| u128::from_le_bytes(b.try_into().unwrap()))
    }
    /// A count of items that each take at least `min` bytes: rejects counts
    /// the remaining input cannot hold, so corrupt data cannot force a huge
    /// allocation.
    fn count(&mut self, min: usize) -> Option<usize> {
        let n = usize::try_from(self.u64()?).ok()?;
        if n.checked_mul(min)? > self.0.len() { None } else { Some(n) }
    }
    fn str(&mut self) -> Option<String> {
        let n = self.count(1)?;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    fn opt_bool(&mut self) -> Option<Option<bool>> {
        match self.u8()? {
            0 => Some(None),
            1 => Some(Some(false)),
            2 => Some(Some(true)),
            _ => None,
        }
    }
}

fn put_pos(w: &mut Writer, p: &Position) {
    w.str(&p.file);
    w.i64(p.line);
    w.i64(p.column);
}

fn get_pos(r: &mut Reader) -> Option<Position> {
    Some(Position { file: r.str()?, line: r.i64()?, column: r.i64()? })
}

fn severity_byte(s: Severity) -> u8 {
    match s {
        Severity::ErrorC => 0,
        Severity::WarningC => 1,
        Severity::InfoC => 2,
        Severity::StyleC => 3,
    }
}

fn severity_of(b: u8) -> Option<Severity> {
    Some(match b {
        0 => Severity::ErrorC,
        1 => Severity::WarningC,
        2 => Severity::InfoC,
        3 => Severity::StyleC,
        _ => return None,
    })
}

fn put_result(w: &mut Writer, res: &CheckResult) {
    w.str(&res.filename);
    w.u64(res.comments.len() as u64);
    for pc in &res.comments {
        put_pos(w, &pc.start_pos);
        put_pos(w, &pc.end_pos);
        w.u8(severity_byte(pc.comment.severity));
        w.i64(pc.comment.code);
        w.str(&pc.comment.message);
        match &pc.fix {
            None => w.u8(0),
            Some(fix) => {
                w.u8(1);
                w.u64(fix.replacements.len() as u64);
                for rep in &fix.replacements {
                    put_pos(w, &rep.start_pos);
                    put_pos(w, &rep.end_pos);
                    w.str(&rep.string);
                    w.i64(rep.precedence);
                    w.u8(match rep.insertion_point {
                        InsertionPoint::InsertBefore => 0,
                        InsertionPoint::InsertAfter => 1,
                    });
                }
            }
        }
    }
}

fn get_result(r: &mut Reader) -> Option<CheckResult> {
    let filename = r.str()?;
    let n = r.count(1)?;
    let mut comments = Vec::with_capacity(n);
    for _ in 0..n {
        let start_pos = get_pos(r)?;
        let end_pos = get_pos(r)?;
        let severity = severity_of(r.u8()?)?;
        let code = r.i64()?;
        let message = r.str()?;
        let fix = match r.u8()? {
            0 => None,
            1 => {
                let m = r.count(1)?;
                let mut replacements = Vec::with_capacity(m);
                for _ in 0..m {
                    let start_pos = get_pos(r)?;
                    let end_pos = get_pos(r)?;
                    let string = r.str()?;
                    let precedence = r.i64()?;
                    let insertion_point = match r.u8()? {
                        0 => InsertionPoint::InsertBefore,
                        1 => InsertionPoint::InsertAfter,
                        _ => return None,
                    };
                    replacements.push(Replacement { start_pos, end_pos, string, precedence, insertion_point });
                }
                Some(Fix { replacements })
            }
            _ => return None,
        };
        comments.push(PositionedComment { start_pos, end_pos, comment: Comment { severity, code, message }, fix });
    }
    Some(CheckResult { filename, comments })
}

// ---- the log of interface calls -------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug)]
enum Dep {
    /// `read_file(external, path)`: `Ok(hash of the contents)` or the policy
    /// refusal message.
    Read { external: Option<bool>, path: String, outcome: Result<u128, String> },
    FindSource { current: String, external: Option<bool>, source_paths: Vec<String>, name: String, found: String },
    /// `get_config(file)`: `(name, hash of the contents)` of the rc or
    /// editorconfig in effect.
    Config { file: String, found: Option<(String, u128)> },
}

fn put_dep(w: &mut Writer, d: &Dep) {
    match d {
        Dep::Read { external, path, outcome } => {
            w.u8(1);
            w.opt_bool(*external);
            w.str(path);
            match outcome {
                Ok(h) => {
                    w.u8(0);
                    w.u128(*h);
                }
                Err(m) => {
                    w.u8(1);
                    w.str(m);
                }
            }
        }
        Dep::FindSource { current, external, source_paths, name, found } => {
            w.u8(2);
            w.str(current);
            w.opt_bool(*external);
            w.u64(source_paths.len() as u64);
            for p in source_paths {
                w.str(p);
            }
            w.str(name);
            w.str(found);
        }
        Dep::Config { file, found } => {
            w.u8(3);
            w.str(file);
            match found {
                None => w.u8(0),
                Some((name, h)) => {
                    w.u8(1);
                    w.str(name);
                    w.u128(*h);
                }
            }
        }
    }
}

fn get_dep(r: &mut Reader) -> Option<Dep> {
    Some(match r.u8()? {
        1 => {
            let external = r.opt_bool()?;
            let path = r.str()?;
            let outcome = match r.u8()? {
                0 => Ok(r.u128()?),
                1 => Err(r.str()?),
                _ => return None,
            };
            Dep::Read { external, path, outcome }
        }
        2 => {
            let current = r.str()?;
            let external = r.opt_bool()?;
            let n = r.count(8)?;
            let mut source_paths = Vec::with_capacity(n);
            for _ in 0..n {
                source_paths.push(r.str()?);
            }
            Dep::FindSource { current, external, source_paths, name: r.str()?, found: r.str()? }
        }
        3 => {
            let file = r.str()?;
            let found = match r.u8()? {
                0 => None,
                1 => Some((r.str()?, r.u128()?)),
                _ => return None,
            };
            Dep::Config { file, found }
        }
        _ => return None,
    })
}

/// A refusal that comes from the `-x` policy (no file is read), so the same
/// call gives the same answer for as long as the options do. Any other error
/// (I/O failure, missing file) is something we cannot verify: do not cache.
fn is_policy_refusal(msg: &str) -> bool {
    msg.contains(" was not specified as input")
}

/// Wraps the real interface and logs what the checker asks it.
struct Recorder<'a> {
    inner: &'a dyn SystemInterface,
    log: Mutex<Vec<Dep>>,
    /// Set when an answer was seen that a later run could not verify.
    unverifiable: Mutex<bool>,
}

impl SystemInterface for Recorder<'_> {
    fn read_file(&self, external: Option<bool>, path: &str) -> Result<String, ErrorMessage> {
        let res = self.inner.read_file(external, path);
        let outcome = match &res {
            Ok(c) => Ok(hash128(c.as_bytes())),
            Err(m) => {
                if !is_policy_refusal(m) {
                    *self.unverifiable.lock().unwrap() = true;
                }
                Err(m.clone())
            }
        };
        self.log.lock().unwrap().push(Dep::Read { external, path: path.to_string(), outcome });
        res
    }

    fn find_source(&self, current: &str, external: Option<bool>, source_paths: &[String], name: &str) -> String {
        let found = self.inner.find_source(current, external, source_paths, name);
        self.log.lock().unwrap().push(Dep::FindSource {
            current: current.to_string(),
            external,
            source_paths: source_paths.to_vec(),
            name: name.to_string(),
            found: found.clone(),
        });
        found
    }

    fn get_config(&self, file: &str) -> Option<(String, String)> {
        let res = self.inner.get_config(file);
        let found = res.as_ref().map(|(n, c)| (n.clone(), hash128(c.as_bytes())));
        self.log.lock().unwrap().push(Dep::Config { file: file.to_string(), found });
        res
    }
}

/// Ask the live interface every recorded question again; true if all answers
/// are the recorded ones.
fn verify(sys: &dyn SystemInterface, deps: &[Dep]) -> bool {
    deps.iter().all(|d| match d {
        Dep::Read { external, path, outcome } => match (sys.read_file(*external, path), outcome) {
            (Ok(c), Ok(h)) => hash128(c.as_bytes()) == *h,
            (Err(m), Err(want)) => is_policy_refusal(&m) && m == *want,
            _ => false,
        },
        Dep::FindSource { current, external, source_paths, name, found } => {
            sys.find_source(current, *external, source_paths, name) == *found
        }
        Dep::Config { file, found } => {
            let now = sys.get_config(file).map(|(n, c)| (n, hash128(c.as_bytes())));
            now == *found
        }
    })
}

// ---- keys and entries ------------------------------------------------------

fn shell_byte(s: Option<Shell>) -> u8 {
    match s {
        None => 0,
        Some(Shell::Ksh) => 1,
        Some(Shell::Sh) => 2,
        Some(Shell::Bash) => 3,
        Some(Shell::Dash) => 4,
        Some(Shell::BusyboxSh) => 5,
    }
}

/// Everything known before the check that can change its result. When in
/// doubt a field is included: an extra field only costs a miss.
fn key_bytes(options: &Options, spec: &CheckSpec) -> Vec<u8> {
    let mut w = Writer::default();
    w.str(env!("CARGO_PKG_VERSION"));
    w.str(SHELLCHECK_VERSION);
    // A rebuilt binary can behave differently under the same version number
    // (development builds), so its size and modification time count too.
    match std::env::current_exe().and_then(std::fs::metadata) {
        Ok(m) => {
            w.u8(1);
            w.u64(m.len());
            let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
            w.u64(t.map_or(0, |d| d.as_secs()));
            w.u64(t.map_or(0, |d| d.subsec_nanos() as u64));
        }
        Err(_) => w.u8(0),
    }
    w.str(&spec.filename);
    w.u128(hash128(spec.script.as_bytes()));
    w.u64(spec.script.len() as u64);
    w.u8(spec.check_sourced as u8);
    w.u8(spec.ignore_rc as u8);
    w.u64(spec.excluded_warnings.len() as u64);
    for c in &spec.excluded_warnings {
        w.i64(*c);
    }
    match &spec.included_warnings {
        None => w.u8(0),
        Some(v) => {
            w.u8(1);
            w.u64(v.len() as u64);
            for c in v {
                w.i64(*c);
            }
        }
    }
    w.u8(shell_byte(spec.shell_type_override));
    w.u8(severity_byte(spec.min_severity));
    w.opt_bool(spec.extended_analysis);
    w.u64(spec.optional_checks.len() as u64);
    for c in &spec.optional_checks {
        w.str(c);
    }
    w.u8(options.external_sources as u8);
    w.u64(options.source_paths.len() as u64);
    for p in &options.source_paths {
        w.str(p);
    }
    match &options.rcfile {
        None => w.u8(0),
        Some(f) => {
            w.u8(1);
            w.str(f);
        }
    }
    w.0
}

fn entry_path(dir: &Path, key: &[u8]) -> PathBuf {
    dir.join(format!("{:032x}.slc", hash128(key)))
}

fn encode_entry(key: &[u8], deps: &[Dep], res: &CheckResult) -> Vec<u8> {
    let mut w = Writer::default();
    w.0.extend_from_slice(MAGIC);
    w.str("");
    w.u64(key.len() as u64);
    w.0.extend_from_slice(key);
    w.u64(deps.len() as u64);
    for d in deps {
        put_dep(&mut w, d);
    }
    put_result(&mut w, res);
    let sum = hash128(&w.0);
    w.u128(sum);
    w.0
}

fn decode_entry(bytes: &[u8], key: &[u8]) -> Option<(Vec<Dep>, CheckResult)> {
    let (body, sum) = bytes.split_at(bytes.len().checked_sub(16)?);
    if hash128(body).to_le_bytes() != sum {
        return None;
    }
    let mut r = Reader(body);
    if r.take(MAGIC.len())? != MAGIC {
        return None;
    }
    r.str()?;
    let klen = r.count(1)?;
    if r.take(klen)? != key {
        return None;
    }
    let n = r.count(1)?;
    let mut deps = Vec::with_capacity(n);
    for _ in 0..n {
        deps.push(get_dep(&mut r)?);
    }
    let res = get_result(&mut r)?;
    if !r.0.is_empty() {
        return None;
    }
    Some((deps, res))
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write to a temporary file in the same directory, then rename it into place,
/// so readers never see a partial entry. Failures are ignored: the cache is an
/// optimisation.
fn store(path: &Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = dir.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&tmp, bytes).is_err() || std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `check_script_owned` with a cache in `cache_dir`.
pub fn check_cached(sys: &dyn SystemInterface, cache_dir: &str, options: &Options, spec: CheckSpec) -> CheckResult {
    let key = key_bytes(options, &spec);
    let path = entry_path(Path::new(cache_dir), &key);
    if let Ok(bytes) = std::fs::read(&path) {
        if let Some((deps, res)) = decode_entry(&bytes, &key) {
            if verify(sys, &deps) {
                return res;
            }
        }
    }
    let rec = Recorder { inner: sys, log: Mutex::new(vec![]), unverifiable: Mutex::new(false) };
    let res = check_script_owned(&rec, spec);
    if !*rec.unverifiable.lock().unwrap() {
        let deps = rec.log.into_inner().unwrap();
        store(&path, &encode_entry(&key, &deps, &res));
    }
    res
}
