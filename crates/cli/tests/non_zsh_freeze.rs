// SPDX-License-Identifier: GPL-3.0-or-later
// DIVERGENCE: shelllint-only regression harness (no ShellCheck counterpart).
//
// Non-zsh freeze. zsh support must not change a single byte of output for the
// other shells. This test runs the built `shelllint` (`-f json1`) over a fixed
// set of scripts under each of sh, bash, dash, ksh, busybox and bash with
// `--bash-version=3.2`, and compares one hash per (script, shell) against the
// recorded one in `tests/freeze/hashes.txt`.
//
// Inputs (listed in `tests/freeze/manifest.txt`, with a hash of each input):
//   * every `script` in `corpus/tests.jsonl` (2105 shell snippets that carry
//     oracle data; the other records in the corpus are Haskell test sources
//     with no script), written out as `c<index>.sh`;
//   * the fixture scripts: `crates/shelllint/tests/fixtures/*.sh`,
//     `crates/formatter/tests/golden/in/*.sh`, `bench/handwritten/*.sh` and
//     `demo/buggy.sh`, written out as `f<index>.sh`.
// Each (script, shell) hash covers two runs: default checks, and `--enable=all`
// (so optional checks are frozen too). Files are linted in one process per run.
//
// If this test fails, a change altered non-zsh output. Fix the change. Do not
// regenerate. The files are generated once, from the pre-zsh tree, with
//     SHELLLINT_FREEZE_UPDATE=1 cargo test -p shelllint --test non_zsh_freeze
// and are not to be regenerated afterwards (only if the input set itself is
// deliberately changed, with the new hashes taken from a tree known to be
// byte-identical to the previous ones).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHELLS: [(&str, &[&str]); 6] = [
    ("sh", &["--shell=sh"]),
    ("bash", &["--shell=bash"]),
    ("dash", &["--shell=dash"]),
    ("ksh", &["--shell=ksh"]),
    ("busybox", &["--shell=busybox"]),
    ("bash32", &["--shell=bash", "--bash-version=3.2"]),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn freeze_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/freeze")
}

/// FNV-1a, 64 bit. Only used to detect changes, not for security.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// (id, file name, contents), in a stable order.
fn inputs() -> Vec<(String, String, Vec<u8>)> {
    let r = root();
    let mut v = Vec::new();
    let jsonl = std::fs::read_to_string(r.join("corpus/tests.jsonl")).unwrap();
    for (i, line) in jsonl.lines().enumerate() {
        let rec: serde_json::Value = serde_json::from_str(line).unwrap();
        if let Some(s) = rec.get("script").and_then(|s| s.as_str()) {
            let name = rec["name"].as_str().unwrap_or("");
            let module = rec["module"].as_str().unwrap_or("");
            v.push((format!("corpus:{i}:{module}:{name}"), format!("c{i}.sh"), s.as_bytes().to_vec()));
        }
    }
    let mut fixtures: Vec<PathBuf> = Vec::new();
    for d in ["crates/shelllint/tests/fixtures", "crates/formatter/tests/golden/in", "bench/handwritten"] {
        for e in std::fs::read_dir(r.join(d)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "sh") {
                fixtures.push(p);
            }
        }
    }
    fixtures.push(r.join("demo/buggy.sh"));
    let mut rel: Vec<String> =
        fixtures.iter().map(|p| p.strip_prefix(&r).unwrap().to_string_lossy().replace('\\', "/")).collect();
    rel.sort();
    for (i, p) in rel.iter().enumerate() {
        v.push((format!("file:{p}"), format!("f{i}.sh"), std::fs::read(r.join(p)).unwrap()));
    }
    v
}

fn run(dir: &Path, extra: &[&str], opts: &[&str], names: &[String]) -> BTreeMap<String, String> {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(["--norc", "-f", "json1"])
        .args(extra)
        .args(opts)
        .args(names)
        .current_dir(dir)
        .env_remove("SHELLCHECK_OPTS")
        .output()
        .unwrap();
    assert!(
        matches!(o.status.code(), Some(0) | Some(1)),
        "shelllint {opts:?} exited {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let mut per: BTreeMap<String, String> = names.iter().map(|n| (n.clone(), String::new())).collect();
    for c in v["comments"].as_array().unwrap() {
        let f = c["file"].as_str().unwrap();
        let s = per.get_mut(f).unwrap_or_else(|| panic!("unexpected file {f}"));
        s.push_str(&c.to_string());
        s.push('\n');
    }
    per
}

/// id -> hashes (one per entry of SHELLS).
fn measure(inputs: &[(String, String, Vec<u8>)]) -> BTreeMap<String, Vec<u64>> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("freeze-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let names: Vec<String> = inputs.iter().map(|i| i.1.clone()).collect();
    for (_, n, c) in inputs {
        std::fs::write(dir.join(n), c).unwrap();
    }
    let mut out: BTreeMap<String, Vec<u64>> = inputs.iter().map(|i| (i.0.clone(), Vec::new())).collect();
    for (_, opts) in SHELLS {
        let a = run(&dir, &[], opts, &names);
        let b = run(&dir, &["--enable=all"], opts, &names);
        for (id, n, _) in inputs {
            let mut s = a[n].clone();
            s.push_str("--enable=all\n");
            s.push_str(&b[n]);
            out.get_mut(id).unwrap().push(fnv(s.as_bytes()));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn manifest_text(inputs: &[(String, String, Vec<u8>)]) -> String {
    let mut s = String::from("# id<TAB>input hash. Generated by non_zsh_freeze.rs, see the header there.\n");
    for (id, _, c) in inputs {
        s.push_str(&format!("{id}\t{:016x}\n", fnv(c)));
    }
    s
}

fn hashes_text(h: &BTreeMap<String, Vec<u64>>) -> String {
    let cols: Vec<&str> = SHELLS.iter().map(|s| s.0).collect();
    let mut s =
        format!("# id<TAB>hash per shell ({}). Generated by non_zsh_freeze.rs, never regenerate.\n", cols.join(" "));
    for (id, v) in h {
        let hs: Vec<String> = v.iter().map(|x| format!("{x:016x}")).collect();
        s.push_str(&format!("{id}\t{}\n", hs.join(" ")));
    }
    s
}

#[test]
fn non_zsh_output_is_frozen() {
    let ins = inputs();
    let manifest = manifest_text(&ins);
    let measured = measure(&ins);
    let hashes = hashes_text(&measured);
    let fd = freeze_dir();
    if std::env::var_os("SHELLLINT_FREEZE_UPDATE").is_some() {
        std::fs::create_dir_all(&fd).unwrap();
        std::fs::write(fd.join("manifest.txt"), manifest).unwrap();
        std::fs::write(fd.join("hashes.txt"), hashes).unwrap();
        eprintln!("freeze: wrote {} inputs", ins.len());
        return;
    }
    let want_manifest = std::fs::read_to_string(fd.join("manifest.txt")).unwrap();
    assert!(
        manifest == want_manifest,
        "the freeze input set differs from tests/freeze/manifest.txt (inputs changed, not behaviour)"
    );
    let want_text = std::fs::read_to_string(fd.join("hashes.txt")).unwrap();
    let want: BTreeMap<&str, &str> =
        want_text.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once('\t')).collect();
    let mut bad = Vec::new();
    for (id, v) in &measured {
        let got: Vec<String> = v.iter().map(|x| format!("{x:016x}")).collect();
        let w: Vec<&str> = want.get(id.as_str()).map(|s| s.split(' ').collect()).unwrap_or_default();
        for (i, (name, _)) in SHELLS.iter().enumerate() {
            if w.get(i).copied() != Some(got[i].as_str()) {
                bad.push(format!("{id} [{name}]"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "non-zsh output changed for {} (file, shell) pairs, e.g. {:?}. Output for sh/bash/dash/ksh/busybox must stay byte-identical.",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
    assert_eq!(want.len(), measured.len());
}
