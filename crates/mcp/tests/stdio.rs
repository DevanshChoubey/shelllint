// SPDX-License-Identifier: GPL-3.0-or-later
//! Drives the real `shellcheck-rust-mcp` binary over stdio.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// Send `messages` (one JSON-RPC message per line), close stdin, and return
/// every reply the server wrote, parsed.
fn session(messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn shellcheck-rust-mcp");
    {
        let mut stdin = child.stdin.take().unwrap();
        for m in messages {
            writeln!(stdin, "{m}").unwrap();
        }
    }
    let out = child.wait_with_output().expect("wait for server");
    assert!(out.status.success(), "server exited with {:?}: {}", out.status, String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad JSON line {l:?}: {e}")))
        .collect()
}

fn reply(replies: &[Value], id: i64) -> &Value {
    replies.iter().find(|r| r["id"] == json!(id)).unwrap_or_else(|| panic!("no reply for id {id}: {replies:?}"))
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}})
}

const SC2086_SCRIPT: &str = "#!/bin/sh\nrm $1\n";

#[test]
fn handshake_list_and_check_script_finds_sc2086() {
    let replies = session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        call(3, "check_script", json!({"script": SC2086_SCRIPT})),
    ]);

    // initialize: negotiated version and tools capability; the notification got no reply.
    let init = &reply(&replies, 1)["result"];
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert!(init["capabilities"]["tools"].is_object());
    assert_eq!(init["serverInfo"]["name"], "shellcheck-rust-mcp");
    assert_eq!(replies.len(), 3, "notification must not be answered: {replies:?}");

    // tools/list: the five tools, each with a description and an input schema.
    let names: Vec<&str> =
        reply(&replies, 2)["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["check_script", "check_file", "explain", "list_checks", "fix_script"]);

    // tools/call check_script: SC2086 on line 2, column 4.
    let result = &reply(&replies, 3)["result"];
    assert_eq!(result["isError"], false);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("SC2086"), "summary: {text}");
    let diags = result["structuredContent"]["diagnostics"].as_array().unwrap();
    let sc2086 = diags.iter().find(|d| d["code"] == "SC2086").expect("SC2086 reported");
    assert_eq!((sc2086["line"].as_i64(), sc2086["column"].as_i64()), (Some(2), Some(4)));
    assert_eq!(sc2086["wiki"], "https://www.shellcheck.net/wiki/SC2086");
    // The fix inserts the quotes as separate replacements around $1.
    let reps = sc2086["fix"]["replacements"].as_array().unwrap();
    assert!(!reps.is_empty() && reps.iter().all(|r| r["text"] == "\""), "replacements: {reps:?}");
}

#[test]
fn fix_script_returns_fixed_script_and_diff() {
    let replies = session(&[call(1, "fix_script", json!({"script": SC2086_SCRIPT}))]);
    let result = &reply(&replies, 1)["result"];
    assert_eq!(result["structuredContent"]["fixed_script"], "#!/bin/sh\nrm \"$1\"\n");
    assert_eq!(result["structuredContent"]["applied"], 1);
    let diff = result["structuredContent"]["diff"].as_str().unwrap();
    assert!(diff.contains("-rm $1") && diff.contains("+rm \"$1\""), "diff: {diff}");
}

#[test]
fn explain_returns_wiki_and_examples() {
    let replies = session(&[call(1, "explain", json!({"code": "SC2086"}))]);
    let result = &reply(&replies, 1)["result"];
    assert_eq!(result["isError"], false);
    let s = &result["structuredContent"];
    assert_eq!(s["wiki"], "https://www.shellcheck.net/wiki/SC2086");
    assert_eq!(s["bad"], "rm $1");
    // The corpus has no non-firing test for SC2086, so there is no good example.
    assert!(s["good"].is_null());
}

#[test]
fn check_file_reads_from_disk() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("mcp-check-file.sh");
    std::fs::write(&path, SC2086_SCRIPT).unwrap();
    let replies = session(&[call(1, "check_file", json!({"path": path.to_str().unwrap()}))]);
    let result = &reply(&replies, 1)["result"];
    assert!(result["content"][0]["text"].as_str().unwrap().contains("SC2086"));
}

#[test]
fn list_checks_filters_and_optional_only() {
    let replies = session(&[
        call(1, "list_checks", json!({"filter": "SC2086"})),
        call(2, "list_checks", json!({"optional": true})),
    ]);
    let all_2086 = &reply(&replies, 1)["result"]["structuredContent"];
    assert_eq!(all_2086["count"], 1);
    assert!(all_2086["checks"][0]["code"] == "SC2086");
    let optional = &reply(&replies, 2)["result"]["structuredContent"];
    assert!(optional["count"].as_i64().unwrap() > 0);
    assert!(optional["checks"].as_array().unwrap().iter().all(|c| c["optional"].is_string()));
}

#[test]
fn bad_arguments_are_tool_errors_not_protocol_errors() {
    let replies = session(&[
        call(1, "check_script", json!({"script": "echo", "exclude": ["nope"]})),
        call(2, "check_script", json!({"script": "echo", "enable": ["no-such-check"]})),
        call(3, "check_script", json!({})),
        call(4, "explain", json!({"code": "SC999999"})),
    ]);
    for id in 1..=4 {
        let r = reply(&replies, id);
        assert!(r.get("error").is_none(), "protocol error for id {id}: {r}");
        assert_eq!(r["result"]["isError"], true, "id {id}: {r}");
    }
}

#[test]
fn protocol_errors_have_json_rpc_codes() {
    let replies = session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "bogus/method"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "no_such_tool"}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "server/discover", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list",
               "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "1999-01-01"}}}),
    ]);
    assert_eq!(reply(&replies, 1)["error"]["code"], -32601);
    assert_eq!(reply(&replies, 2)["error"]["code"], -32602);
    assert_eq!(reply(&replies, 3)["result"], json!({}));
    let discover = &reply(&replies, 4)["result"];
    assert_eq!(discover["supportedVersions"][0], "2026-07-28");
    assert_eq!(reply(&replies, 5)["error"]["code"], -32022);
}

#[test]
fn unparsable_line_gets_parse_error_and_server_keeps_going() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{{not json").unwrap();
        writeln!(stdin, "{}", json!({"jsonrpc": "2.0", "id": 9, "method": "ping"})).unwrap();
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let lines: Vec<Value> =
        String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[1]["id"], 9);
}

/// A tiny deterministic generator (xorshift), so the fuzz test needs no crate
/// and every run sends the same messages.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
}

const NASTY_STRINGS: &[&str] = &[
    "",
    " ",
    "a\u{e9}",
    "\u{e9}",
    "\u{1F600}",
    "S\u{e9}2086",
    "sc\u{1F600}",
    "SC\u{663}\u{664}",
    "\u{FF33}\u{FF23}2086",
    "SC",
    "SC-1",
    "SC99999999999999999999",
    "2086,\u{e9},SC2034",
    "\u{0}",
    "\u{FEFF}#!/bin/sh\necho $1",
    "\t\u{85}\u{2028}",
    "echo \"\u{1F600}\" $x\n\tcat <<EOF\n\u{e9}\nEOF",
    "all",
    "../../../../etc/passwd",
    "/nonexistent/\u{e9}.sh",
    "/dev/null",
    "/",
    "sh",
    "bash\u{e9}",
    "style",
    "\u{d7ff}\u{e000}\u{10ffff}",
];

fn nasty_value(rng: &mut Rng, depth: u32) -> Value {
    match rng.next() % if depth > 2 { 5 } else { 8 } {
        0 | 1 | 2 => json!(rng.pick(NASTY_STRINGS)),
        3 => json!(null),
        4 => rng.pick(&[json!(true), json!(-1), json!(1.5e300), json!(u64::MAX), json!(0)]).clone(),
        5 => Value::Array((0..rng.next() % 4).map(|_| nasty_value(rng, depth + 1)).collect()),
        6 => json!([rng.pick(NASTY_STRINGS), rng.pick(NASTY_STRINGS)]),
        _ => json!({ *rng.pick(NASTY_STRINGS): nasty_value(rng, depth + 1) }),
    }
}

/// Many malformed or unicode tool arguments (and raw junk lines) over stdio:
/// the server must answer every request and stay alive to answer a final ping.
#[test]
fn fuzzed_malformed_and_unicode_arguments_never_kill_the_server() {
    const TOOLS: &[&str] = &["check_script", "check_file", "explain", "list_checks", "fix_script"];
    const KEYS: &[&str] = &["script", "path", "code", "exclude", "enable", "shell", "severity", "filter", "optional"];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut input: Vec<u8> = Vec::new();
    let mut ids = Vec::new();
    for id in 1..=400i64 {
        let name = *rng.pick(TOOLS);
        let mut args = serde_json::Map::new();
        for _ in 0..rng.next() % 4 {
            args.insert(rng.pick(KEYS).to_string(), nasty_value(&mut rng, 0));
        }
        // Keep the required argument present (and a string) half the time so the
        // option parsing is reached, not only the "missing argument" path.
        if rng.next() % 2 == 0 {
            let key = match name {
                "check_script" | "fix_script" => "script",
                "check_file" => "path",
                "explain" => "code",
                _ => "filter",
            };
            args.insert(key.into(), json!(rng.pick(NASTY_STRINGS)));
        }
        let arguments = match rng.next() % 10 {
            0 => nasty_value(&mut rng, 0),
            _ => Value::Object(args),
        };
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
                         "params": {"name": name, "arguments": arguments}});
        input.extend(msg.to_string().as_bytes());
        input.push(b'\n');
        ids.push(id);
        // Interleave raw junk: invalid UTF-8, bad JSON, truncated messages.
        match rng.next() % 8 {
            0 => input.extend(b"\xff\xfe\xc3\x28 junk\n"),
            1 => input.extend(b"{\"jsonrpc\":\"2.0\",\"id\":\n"),
            2 => input.extend("[\"\u{1F600}\"]\n".as_bytes()),
            _ => {}
        }
    }
    input.extend(json!({"jsonrpc": "2.0", "id": 100_000, "method": "ping"}).to_string().as_bytes());
    input.push(b'\n');

    let mut child = Command::new(env!("CARGO_BIN_EXE_shellcheck-rust-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "server died: {:?}\n{stderr}", out.status);
    // The server catches handler panics, but none should happen at all.
    assert!(!stderr.contains("panicked"), "a handler panicked:\n{stderr}");
    let replies: Vec<Value> =
        String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    for id in ids {
        let r = reply(&replies, id);
        assert!(r["result"]["isError"].is_boolean(), "id {id} is not a tool result: {r}");
    }
    assert_eq!(reply(&replies, 100_000)["result"], json!({}));
}
