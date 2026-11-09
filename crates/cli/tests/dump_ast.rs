// SPDX-License-Identifier: GPL-3.0-or-later
// Tests for `--dump-ast` (issue #2696), an extension beyond ShellCheck.

use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> &J {
        match self {
            J::Obj(v) => v.iter().find(|(n, _)| n == k).map(|(_, v)| v).unwrap_or_else(|| panic!("no key {k}")),
            _ => panic!("not an object"),
        }
    }
    fn str(&self) -> &str {
        match self {
            J::Str(s) => s,
            _ => panic!("not a string: {self:?}"),
        }
    }
    fn num(&self) -> i64 {
        match self {
            J::Num(n) => *n as i64,
            _ => panic!("not a number"),
        }
    }
    fn arr(&self) -> &[J] {
        match self {
            J::Arr(v) => v,
            _ => panic!("not an array"),
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn lit(&mut self, s: &str) {
        assert!(self.b[self.i..].starts_with(s.as_bytes()), "expected {s} at {}", self.i);
        self.i += s.len();
    }
    fn value(&mut self) -> J {
        self.ws();
        match self.b[self.i] {
            b'n' => {
                self.lit("null");
                J::Null
            }
            b't' => {
                self.lit("true");
                J::Bool(true)
            }
            b'f' => {
                self.lit("false");
                J::Bool(false)
            }
            b'"' => J::Str(self.string()),
            b'[' => {
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.b[self.i] == b']' {
                    self.i += 1;
                    return J::Arr(v);
                }
                loop {
                    v.push(self.value());
                    self.ws();
                    match self.b[self.i] {
                        b',' => self.i += 1,
                        b']' => {
                            self.i += 1;
                            return J::Arr(v);
                        }
                        c => panic!("bad array char {}", c as char),
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut v = vec![];
                self.ws();
                if self.b[self.i] == b'}' {
                    self.i += 1;
                    return J::Obj(v);
                }
                loop {
                    self.ws();
                    let k = self.string();
                    self.ws();
                    self.lit(":");
                    v.push((k, self.value()));
                    self.ws();
                    match self.b[self.i] {
                        b',' => self.i += 1,
                        b'}' => {
                            self.i += 1;
                            return J::Obj(v);
                        }
                        c => panic!("bad object char {}", c as char),
                    }
                }
            }
            _ => {
                let s = self.i;
                while self.i < self.b.len() && (self.b[self.i] == b'-' || self.b[self.i].is_ascii_digit()) {
                    self.i += 1;
                }
                J::Num(std::str::from_utf8(&self.b[s..self.i]).unwrap().parse().unwrap())
            }
        }
    }
    fn string(&mut self) -> String {
        self.lit("\"");
        let mut out = Vec::new();
        loop {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).unwrap(),
                b'\\' => {
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'u' => {
                            let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap();
                            self.i += 4;
                            let ch = char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap();
                            out.extend_from_slice(ch.to_string().as_bytes());
                        }
                        _ => panic!("bad escape"),
                    }
                }
                c => {
                    assert!(c >= 0x20, "raw control character in string");
                    out.push(c);
                }
            }
        }
    }
}

fn parse_json(s: &str) -> J {
    let mut p = P { b: s.as_bytes(), i: 0 };
    let v = p.value();
    p.ws();
    assert_eq!(p.i, s.len(), "trailing data after JSON document");
    v
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shelllint-dump-ast-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_shelllint"))
        .args(args)
        .env_remove("SHELLCHECK_OPTS")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn write(dir: &std::path::Path, name: &str, contents: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, contents).unwrap();
    p.to_string_lossy().into_owned()
}

fn walk<'a>(n: &'a J, f: &mut dyn FnMut(&'a J)) {
    f(n);
    for c in n.get("children").arr() {
        walk(c, f);
    }
}

fn pos(n: &J) -> (i64, i64, i64, i64) {
    let (s, e) = (n.get("start"), n.get("end"));
    (s.get("line").num(), s.get("column").num(), e.get("line").num(), e.get("column").num())
}

#[test]
fn single_file_is_one_document() {
    let d = tmpdir("single");
    let f = write(&d, "a.sh", "#!/bin/sh\necho hi\nx=1\n");
    let (code, out, _) = run(&["--dump-ast", &f]);
    assert_eq!(code, 0);
    let doc = parse_json(&out);
    assert_eq!(doc.get("file").str(), f);
    assert_eq!(*doc.get("parsed"), J::Bool(true));
    let root = doc.get("ast");
    // The parser wraps the script in a T_Annotation for directives.
    assert!(["T_Script", "T_Annotation"].contains(&root.get("type").str()));
    // Compact output: no pretty-printing whitespace between tokens.
    assert!(out.starts_with("{\"file\":"));

    let mut types = vec![];
    let mut ids = vec![];
    walk(root, &mut |n| {
        types.push(n.get("type").str().to_string());
        ids.push(n.get("id").num());
    });
    for t in ["T_Script", "T_SimpleCommand", "T_NormalWord", "T_Literal", "T_Assignment"] {
        assert!(types.iter().any(|x| x == t), "missing {t} in {types:?}");
    }
    ids.sort();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n, "node ids are not unique");
}

#[test]
fn positions_for_a_small_script() {
    let d = tmpdir("pos");
    let f = write(&d, "p.sh", "echo hi\n  ls -l\n");
    let (code, out, _) = run(&["--dump-ast", &f]);
    assert_eq!(code, 0);
    let doc = parse_json(&out);
    let mut lits = vec![];
    let mut cmds = vec![];
    walk(doc.get("ast"), &mut |n| {
        match n.get("type").str() {
            "T_Literal" => lits.push((n.get("value").str().to_string(), pos(n))),
            "T_SimpleCommand" => cmds.push(pos(n)),
            _ => {}
        }
        // Every node starts at or before the point where it ends.
        let (sl, sc, el, ec) = pos(n);
        assert!((sl, sc) <= (el, ec), "{n:?}");
    });
    let find = |v: &str| lits.iter().find(|(s, _)| s == v).unwrap_or_else(|| panic!("no literal {v}")).1;
    assert_eq!(find("echo"), (1, 1, 1, 5));
    assert_eq!(find("hi"), (1, 6, 1, 8));
    assert_eq!(find("ls"), (2, 3, 2, 5));
    assert_eq!(find("-l"), (2, 6, 2, 8));
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0].0, 1);
    assert_eq!(cmds[1].0, 2);
    assert_eq!(cmds[1].1, 3);
}

#[test]
fn many_files_make_an_array_and_status_reflects_failures() {
    let d = tmpdir("many");
    let a = write(&d, "a.sh", "echo a\n");
    let b = write(&d, "b.sh", "echo b\n");
    let (code, out, _) = run(&["--dump-ast", &a, &b]);
    assert_eq!(code, 0);
    let docs = parse_json(&out);
    assert_eq!(docs.arr().len(), 2);
    assert_eq!(docs.arr()[0].get("file").str(), a);
    assert_eq!(docs.arr()[1].get("file").str(), b);

    let bad = write(&d, "bad.sh", "if true; then\n  echo x\n");
    let (code, out, _) = run(&["--dump-ast", &a, &bad]);
    assert_eq!(code, 1);
    let docs = parse_json(&out);
    assert_eq!(*docs.arr()[0].get("parsed"), J::Bool(true));
    let broken = &docs.arr()[1];
    assert_eq!(*broken.get("parsed"), J::Bool(false));
    assert_eq!(*broken.get("ast"), J::Null);
    assert!(!broken.get("errors").arr().is_empty());
    assert!(broken.get("errors").arr()[0].get("code").num() >= 1000);
}

#[test]
fn replaces_the_normal_report_and_escapes_strings() {
    let d = tmpdir("esc");
    // `$x` unquoted would normally warn; the dump prints no report.
    let f = write(&d, "e.sh", "echo $x \"a\\\"b\" 'q\tq'\n");
    let (code, out, _) = run(&["--dump-ast", "-f", "json", &f]);
    assert_eq!(code, 0);
    assert!(!out.contains("\"level\""), "{out}");
    let doc = parse_json(&out);
    let mut vals = vec![];
    walk(doc.get("ast"), &mut |n| {
        if n.get("type").str() == "T_SingleQuoted" {
            vals.push(n.get("value").str().to_string());
        }
    });
    assert_eq!(vals, vec!["q\tq".to_string()]);
}

#[test]
fn unreadable_file_exits_2() {
    let (code, out, err) = run(&["--dump-ast", "/nonexistent/x.sh"]);
    assert_eq!(code, 2);
    assert_eq!(*parse_json(&out).get("parsed"), J::Bool(false));
    assert!(err.contains("/nonexistent/x.sh"), "{err}");
}
