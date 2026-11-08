// SPDX-License-Identifier: GPL-3.0-or-later
//! Drives the real `shelllint-lsp` binary over stdio JSON-RPC.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(20);

struct Lsp {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    /// Messages read while waiting for something else.
    stash: Vec<Value>,
    next_id: i64,
}

impl Lsp {
    fn start(init_options: Value) -> Lsp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shelllint-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn shelllint-lsp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                let mut len = None;
                let mut line = String::new();
                loop {
                    line.clear();
                    match r.read_line(&mut line) {
                        Ok(0) | Err(_) => return,
                        Ok(_) => {}
                    }
                    let t = line.trim_end();
                    if t.is_empty() {
                        break;
                    }
                    if let Some(v) = t.strip_prefix("Content-Length:") {
                        len = v.trim().parse::<usize>().ok();
                    }
                }
                let Some(n) = len else { return };
                let mut body = vec![0u8; n];
                if r.read_exact(&mut body).is_err() {
                    return;
                }
                let v: Value = serde_json::from_slice(&body).expect("server sent invalid JSON");
                if tx.send(v).is_err() {
                    return;
                }
            }
        });
        let mut lsp = Lsp { child, stdin, rx, stash: vec![], next_id: 1 };
        let id = lsp.request(
            "initialize",
            json!({"processId": null, "rootUri": null, "capabilities": {},
                   "initializationOptions": init_options}),
        );
        assert_eq!(id["result"]["capabilities"]["hoverProvider"], true, "{id}");
        lsp.notify("initialized", json!({}));
        lsp
    }

    fn send(&mut self, v: &Value) {
        let body = v.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Send a request and wait for its response (notifications are stashed).
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let m = self.recv_until(deadline);
            if m.get("id") == Some(&json!(id)) && m.get("method").is_none() {
                return m;
            }
            self.stash.push(m);
        }
    }

    fn recv_until(&mut self, deadline: Instant) -> Value {
        let left = deadline.saturating_duration_since(Instant::now());
        self.rx.recv_timeout(left).unwrap_or_else(|_| panic!("timed out waiting for a server message"))
    }

    /// Wait for a `publishDiagnostics` for `uri` that satisfies `pred`.
    fn diagnostics(&mut self, uri: &str, pred: impl Fn(&Value) -> bool) -> (Value, Duration) {
        let started = Instant::now();
        let deadline = started + TIMEOUT;
        let is_match = |m: &Value| {
            m["method"] == "textDocument/publishDiagnostics"
                && m["params"]["uri"] == uri
                && pred(&m["params"]["diagnostics"])
        };
        if let Some(i) = self.stash.iter().position(is_match) {
            return (self.stash.remove(i), started.elapsed());
        }
        loop {
            let m = self.recv_until(deadline);
            if is_match(&m) {
                return (m, started.elapsed());
            }
            self.stash.push(m);
        }
    }

    fn shutdown(mut self) {
        let _ = self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "shellscript", "version": 1, "text": text}}),
    );
}

/// Apply single-line (ASCII) TextEdits to `line`, last edit first.
fn apply_to_line(line: &str, edits: &[Value]) -> String {
    let mut out = line.to_string();
    let mut sorted: Vec<&Value> = edits.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(e["range"]["start"]["character"].as_u64().unwrap()));
    for e in sorted {
        let s = e["range"]["start"]["character"].as_u64().unwrap() as usize;
        let t = e["range"]["end"]["character"].as_u64().unwrap() as usize;
        out.replace_range(s..t, e["newText"].as_str().unwrap());
    }
    out
}

fn has_code(diags: &Value, code: &str) -> bool {
    diags.as_array().unwrap().iter().any(|d| d["code"] == code)
}

fn file_uri(name: &str) -> String {
    // A real file URI on every platform (file:///C:/... on Windows).
    lsp_types::Url::from_file_path(std::env::temp_dir().join(name)).unwrap().to_string()
}

#[test]
fn publishes_diagnostics_on_open_and_serves_code_actions_and_hover() {
    let mut lsp = Lsp::start(json!({}));
    let uri = "untitled:one.sh";
    open(&mut lsp, uri, "#!/bin/sh\necho $foo\n");
    let (msg, _) = lsp.diagnostics(uri, |d| has_code(d, "SC2086"));
    let d = msg["params"]["diagnostics"].as_array().unwrap().iter().find(|d| d["code"] == "SC2086").unwrap().clone();
    assert_eq!(d["range"], json!({"start": {"line": 1, "character": 5}, "end": {"line": 1, "character": 9}}));
    // SC2086 is an info-level comment: LSP Information (3).
    assert_eq!(d["severity"], 3);
    assert_eq!(d["source"], "shellcheck");
    assert_eq!(d["codeDescription"]["href"], "https://www.shellcheck.net/wiki/SC2086");

    // Quick fix from ShellCheck's own fix, plus "disable for this line".
    let resp = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri}, "range": d["range"], "context": {"diagnostics": [d]}}),
    );
    let actions = resp["result"].as_array().expect("code action list");
    let titles: Vec<&str> = actions.iter().map(|a| a["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Apply ShellCheck fix for SC2086"), "titles: {titles:?}");
    assert!(titles.contains(&"Disable SC2086 for this line"), "titles: {titles:?}");
    let fix = actions.iter().find(|a| a["title"] == "Apply ShellCheck fix for SC2086").unwrap();
    assert_eq!(fix["kind"], "quickfix");
    // ShellCheck's fix is two insertions of a double quote, around $foo.
    let edits = fix["edit"]["changes"][uri].as_array().unwrap();
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert_eq!(apply_to_line("echo $foo", edits), "echo \"$foo\"");
    let disable = actions.iter().find(|a| a["title"] == "Disable SC2086 for this line").unwrap();
    assert_eq!(disable["edit"]["changes"][uri][0]["newText"], "# shellcheck disable=SC2086\n");
    assert_eq!(disable["edit"]["changes"][uri][0]["range"]["start"], json!({"line": 1, "character": 0}));

    // Hover on the diagnostic: message and wiki link.
    let hover = lsp
        .request("textDocument/hover", json!({"textDocument": {"uri": uri}, "position": {"line": 1, "character": 6}}));
    let value = hover["result"]["contents"]["value"].as_str().expect("hover markdown");
    assert!(value.contains("SC2086") && value.contains("https://www.shellcheck.net/wiki/SC2086"), "{value}");
    lsp.shutdown();
}

#[test]
fn incremental_change_clears_diagnostic() {
    let mut lsp = Lsp::start(json!({}));
    let uri = "untitled:two.sh";
    open(&mut lsp, uri, "#!/bin/sh\necho $foo\n");
    lsp.diagnostics(uri, |d| has_code(d, "SC2086"));
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2},
               "contentChanges": [{"range": {"start": {"line": 1, "character": 5}, "end": {"line": 1, "character": 9}},
                                   "text": "\"$foo\""}]}),
    );
    // Other comments may remain (for example an unassigned variable); SC2086 must not.
    let (msg, _) = lsp.diagnostics(uri, |d| !has_code(d, "SC2086"));
    assert_eq!(msg["params"]["version"], 2);
    lsp.shutdown();
}

#[test]
fn shell_directive_and_dialect_setting_are_respected() {
    let mut lsp = Lsp::start(json!({}));
    // `[[` is not POSIX: flagged under a sh shebang...
    let uri = "untitled:dialect.sh";
    open(&mut lsp, uri, "#!/bin/sh\n[[ -n \"$x\" ]] && echo hi\n");
    lsp.diagnostics(uri, |d| has_code(d, "SC3010"));
    // ...and accepted once a directive says bash.
    let uri2 = "untitled:directive.sh";
    open(&mut lsp, uri2, "#!/bin/sh\n# shellcheck shell=bash\n[[ -n \"$x\" ]] && echo hi\n");
    let (msg, _) = lsp.diagnostics(uri2, |_| true);
    assert!(!has_code(&msg["params"]["diagnostics"], "SC3010"), "{msg}");
    lsp.shutdown();
}

#[test]
fn honours_shellcheckrc_next_to_the_file() {
    let dir = std::env::temp_dir().join(format!("lsp-rc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".shellcheckrc"), "disable=SC2086\n").unwrap();
    let mut lsp = Lsp::start(json!({}));
    let uri = file_uri(&format!("lsp-rc-{}/rc.sh", std::process::id()));
    open(&mut lsp, &uri, "#!/bin/sh\necho $foo\n");
    let (msg, _) = lsp.diagnostics(&uri, |_| true);
    assert!(!has_code(&msg["params"]["diagnostics"], "SC2086"), "{msg}");
    lsp.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exclude_setting_from_initialization_options() {
    let mut lsp = Lsp::start(json!({"exclude": ["SC2086"], "dialect": "bash"}));
    let uri = "untitled:excl.sh";
    open(&mut lsp, uri, "#!/bin/bash\necho $foo\n");
    let (msg, _) = lsp.diagnostics(uri, |_| true);
    assert!(!has_code(&msg["params"]["diagnostics"], "SC2086"), "{msg}");
    lsp.shutdown();
}

/// 5000 lines of realistic shell with a few problems on every 50th line.
fn big_script() -> String {
    let mut s = String::from("#!/bin/sh\n");
    for i in 0..4999 {
        if i % 50 == 0 {
            s.push_str(&format!("echo $v{i}\n"));
        } else {
            s.push_str(&format!("v{i}=\"value {i}\"\n"));
        }
    }
    s
}

#[test]
fn five_thousand_line_file_latency() {
    let mut lsp = Lsp::start(json!({}));
    let uri = "untitled:big.sh";
    let text = big_script();
    let (_, open_t) = {
        let started = Instant::now();
        open(&mut lsp, uri, &text);
        let r = lsp.diagnostics(uri, |d| has_code(d, "SC2086"));
        (r.0, started.elapsed())
    };

    // Edit near the top: time until the (debounced) diagnostics arrive.
    let started = Instant::now();
    lsp.notify(
        "textDocument/didChange",
        json!({"textDocument": {"uri": uri, "version": 2},
               "contentChanges": [{"range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 0}},
                                   "text": "x=1\n"}]}),
    );
    let (_, edit_t) = lsp.diagnostics(uri, |d| has_code(d, "SC2086"));
    let edit_total = started.elapsed();

    // Code action on a diagnostic far down the file.
    let started = Instant::now();
    let resp = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri}, "range": {"start": {"line": 3000, "character": 0}, "end": {"line": 3000, "character": 0}}, "context": {"diagnostics": []}}),
    );
    let ca_t = started.elapsed();
    assert!(resp["result"].is_array());

    eprintln!(
        "LATENCY 5000-line file: open->diagnostics {:?}; edit->diagnostics (incl. 250ms debounce) {:?} (publish {:?}); codeAction {:?}",
        open_t, edit_total, edit_t, ca_t
    );
    assert!(open_t < Duration::from_secs(5), "open took {open_t:?}");
    assert!(edit_total < Duration::from_secs(5), "edit took {edit_total:?}");
    lsp.shutdown();
}

#[test]
fn unknown_request_gets_method_not_found() {
    let mut lsp = Lsp::start(json!({}));
    let resp = lsp.request("textDocument/completion", json!({}));
    assert_eq!(resp["error"]["code"], -32601);
    lsp.shutdown();
}

impl Lsp {
    fn send_raw(&mut self, bytes: &[u8]) {
        self.stdin.write_all(bytes).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Wait for the JSON-RPC error with a null id and return its code.
    fn null_id_error(&mut self) -> i64 {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let m = self.recv_until(deadline);
            if m.get("id") == Some(&Value::Null) && m.get("error").is_some() {
                return m["error"]["code"].as_i64().unwrap();
            }
            self.stash.push(m);
        }
    }

    fn still_serving(&mut self) {
        let resp = self.request("textDocument/completion", json!({}));
        assert_eq!(resp["error"]["code"], -32601);
    }
}

fn framed(body: &[u8]) -> Vec<u8> {
    let mut v = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    v.extend_from_slice(body);
    v
}

#[test]
fn bad_json_gets_parse_error_and_server_keeps_serving() {
    let mut lsp = Lsp::start(json!({}));
    lsp.send_raw(&framed(b"{oops"));
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.still_serving();
    lsp.shutdown();
}

#[test]
fn invalid_utf8_gets_parse_error_and_server_keeps_serving() {
    let mut lsp = Lsp::start(json!({}));
    lsp.send_raw(&framed(b"{\"jsonrpc\":\"2.0\",\"method\":\"x\xff\"}"));
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.still_serving();
    lsp.shutdown();
}

#[test]
fn json_array_gets_parse_error_and_server_keeps_serving() {
    let mut lsp = Lsp::start(json!({}));
    lsp.send_raw(&framed(b"[1,2,3]"));
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.still_serving();
    lsp.shutdown();
}

#[test]
fn bad_header_gets_parse_error_and_server_keeps_serving() {
    let mut lsp = Lsp::start(json!({}));
    lsp.send_raw(b"Content-Length: abc\r\n\r\n");
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.send_raw(b"Bogus header line\r\n\r\n");
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.send_raw(b"Content-Type: x\r\n\r\n");
    assert_eq!(lsp.null_id_error(), -32700);
    lsp.still_serving();
    lsp.shutdown();
}

#[test]
fn valid_json_that_is_not_a_message_gets_invalid_request() {
    let mut lsp = Lsp::start(json!({}));
    lsp.send_raw(&framed(b"{\"hello\":1}"));
    assert_eq!(lsp.null_id_error(), -32600);
    lsp.still_serving();
    lsp.shutdown();
}
