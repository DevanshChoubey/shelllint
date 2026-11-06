// SPDX-License-Identifier: GPL-3.0-or-later
//! MCP server for the shellcheck-rust linter.
//!
//! The transport is stdio: one JSON-RPC 2.0 message per line. The protocol
//! layer is hand-written (about 150 lines) because the surface is small:
//! initialize / server/discover, ping, tools/list and tools/call. See
//! docs/mcp.md for the client configuration.

pub mod catalog;
pub mod tools;

use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};

/// Name and version reported to clients. The name is the binary name.
pub const SERVER_NAME: &str = "shellcheck-rust-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Protocol revisions this server speaks, newest first. `2026-07-28` is
/// stateless: the client sends its version in each request's `_meta` and
/// discovers the server with `server/discover`. `2025-11-25` and
/// `2025-06-18` negotiate with `initialize`.
pub const SUPPORTED_VERSIONS: &[&str] = &["2026-07-28", "2025-11-25", "2025-06-18"];

/// Answer to `initialize` for a client that asks for a revision we do not know.
const INITIALIZE_FALLBACK: &str = "2025-11-25";

/// Key of the per-request protocol version in `_meta` (2026-07-28 and later).
const PROTOCOL_VERSION_META_KEY: &str = "io.modelcontextprotocol/protocolVersion";

pub const INSTRUCTIONS: &str = "ShellCheck for shell scripts. Call check_script on script text \
(or check_file on a path) before you finish editing a shell script; use explain to understand a \
code, fix_script to apply automatic fixes, and list_checks to find optional checks.";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// A JSON-RPC error response, minus the id.
#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError { code, message: message.into(), data: None }
    }
}

/// Longest accepted line, in bytes. A script is at most 1 MiB, which JSON
/// escaping can grow to about 6 MiB; anything far beyond that is refused.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

/// Read JSON-RPC messages from `input` and write responses to `output` until
/// `input` ends. Notifications get no response. A line that is not valid
/// UTF-8 or is too long gets a parse error, like any other malformed line;
/// only an I/O error on the streams themselves ends the loop.
pub fn serve<R: BufRead, W: Write>(mut input: R, mut output: W) -> io::Result<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = input.by_ref().take(MAX_LINE_BYTES as u64 + 1).read_until(b'\n', &mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let reply = if buf.len() > MAX_LINE_BYTES && buf.last() != Some(&b'\n') {
            skip_rest_of_line(&mut input)?;
            Some(parse_error(format!("Parse error: message longer than {MAX_LINE_BYTES} bytes")))
        } else {
            match std::str::from_utf8(&buf) {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => handle_line(line),
                Err(e) => Some(parse_error(format!("Parse error: invalid UTF-8: {e}"))),
            }
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
}

/// Discard input up to and including the next newline (or the end).
fn skip_rest_of_line<R: BufRead>(input: &mut R) -> io::Result<()> {
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok(());
        }
        match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => {
                input.consume(i + 1);
                return Ok(());
            }
            None => {
                let len = chunk.len();
                input.consume(len);
            }
        }
    }
}

fn parse_error(message: String) -> Value {
    error_reply(Value::Null, RpcError::new(PARSE_ERROR, message))
}

/// Handle one raw line. Returns the reply to send, if any. A panic while
/// handling it (a bug) becomes an internal error reply, so one bad request
/// never takes the server down.
pub fn handle_line(line: &str) -> Option<Value> {
    let msg = match serde_json::from_str::<Value>(line) {
        Ok(msg) => msg,
        Err(e) => return Some(parse_error(format!("Parse error: {e}"))),
    };
    match std::panic::catch_unwind(|| handle_message(&msg)) {
        Ok(reply) => reply,
        Err(_) => {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            Some(error_reply(id, RpcError::new(INTERNAL_ERROR, "Internal error while handling the request")))
        }
    }
}

fn handle_message(msg: &Value) -> Option<Value> {
    // Batches were removed in 2025-06-18 and are not supported.
    let Some(obj) = msg.as_object() else {
        let message = if msg.is_array() { "Batch requests are not supported" } else { "Request must be an object" };
        return Some(error_reply(Value::Null, RpcError::new(INVALID_REQUEST, message)));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A reply from the client to a request we never sent: ignore it.
        if obj.contains_key("result") || obj.contains_key("error") {
            return None;
        }
        return Some(error_reply(id.unwrap_or(Value::Null), RpcError::new(INVALID_REQUEST, "Missing method")));
    };
    let Some(id) = id else {
        // Notification (notifications/initialized, notifications/cancelled, ...): no reply.
        return None;
    };
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match dispatch(method, &params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(e) => error_reply(id, e),
    })
}

fn error_reply(id: Value, e: RpcError) -> Value {
    let mut error = json!({"code": e.code, "message": e.message});
    if let Some(data) = e.data {
        error["data"] = data;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": error})
}

fn dispatch(method: &str, params: &Value) -> Result<Value, RpcError> {
    if method != "initialize" {
        check_request_version(params)?;
    }
    match method {
        "initialize" => Ok(initialize(params)),
        "server/discover" => Ok(discover()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools::definitions(), "resultType": "complete"})),
        "tools/call" => tools_call(params),
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("Method not found: {method}"))),
    }
}

/// A 2026-07-28 request names its protocol version in `_meta`. Reject versions we do not speak.
fn check_request_version(params: &Value) -> Result<(), RpcError> {
    let requested = params.pointer("/_meta").and_then(|m| m.get(PROTOCOL_VERSION_META_KEY)).and_then(Value::as_str);
    match requested {
        Some(v) if !SUPPORTED_VERSIONS.contains(&v) => Err(RpcError {
            code: UNSUPPORTED_PROTOCOL_VERSION,
            message: format!("Unsupported protocol version: {v}"),
            data: Some(json!({"requested": v, "supported": SUPPORTED_VERSIONS})),
        }),
        _ => Ok(()),
    }
}

fn server_info() -> Value {
    json!({"name": SERVER_NAME, "version": SERVER_VERSION})
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = if SUPPORTED_VERSIONS.contains(&requested) { requested } else { INITIALIZE_FALLBACK };
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": server_info(),
        "instructions": INSTRUCTIONS,
    })
}

fn discover() -> Value {
    json!({
        "resultType": "complete",
        "cacheScope": "public",
        "ttlMs": 3_600_000,
        "supportedVersions": SUPPORTED_VERSIONS,
        "capabilities": {"tools": {}},
        "instructions": INSTRUCTIONS,
    })
}

fn tools_call(params: &Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "tools/call needs a string 'name'"))?;
    if !tools::exists(name) {
        return Err(RpcError::new(INVALID_PARAMS, format!("Unknown tool: {name}")));
    }
    let args = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(a @ Value::Object(_)) => a.clone(),
        Some(_) => return Ok(tools::ToolOutput::error("'arguments' must be an object").to_json()),
    };
    // A panic inside a tool (a checker bug on some input) is reported as a
    // failed call the model can read, not a dead server.
    let out = std::panic::catch_unwind(|| tools::call(name, &args)).unwrap_or_else(|_| {
        tools::ToolOutput::error(format!("Internal error: the {name} tool crashed on this input. Please report it."))
    });
    Ok(out.to_json())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_known_version_and_falls_back() {
        let known =
            handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#);
        assert_eq!(known.unwrap()["result"]["protocolVersion"], "2025-06-18");
        let unknown =
            handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#);
        assert_eq!(unknown.unwrap()["result"]["protocolVersion"], INITIALIZE_FALLBACK);
    }

    #[test]
    fn notifications_get_no_reply_and_bad_input_is_an_error() {
        assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        assert_eq!(handle_line("{oops").unwrap()["error"]["code"], PARSE_ERROR);
        assert_eq!(handle_line("[]").unwrap()["error"]["code"], INVALID_REQUEST);
        // Non-object arguments are a failed tool call, not a protocol error.
        let r = handle_line(r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"explain","arguments":[1]}}"#);
        assert_eq!(r.unwrap()["result"]["isError"], true);
        let r = handle_line(r#"{"jsonrpc":"2.0","id":7,"method":"nope"}"#).unwrap();
        assert_eq!((r["id"].clone(), r["error"]["code"].clone()), (json!(7), json!(METHOD_NOT_FOUND)));
    }

    #[test]
    fn serve_survives_invalid_utf8_and_overlong_lines() {
        let mut input = b"\xff\xfe{}\n".to_vec();
        input.extend(std::iter::repeat_n(b'x', MAX_LINE_BYTES + 10));
        input.extend(b"\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n");
        let mut out = Vec::new();
        serve(&input[..], &mut out).unwrap();
        let replies: Vec<Value> =
            String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(replies.len(), 3, "{replies:?}");
        assert_eq!(replies[0]["error"]["code"], PARSE_ERROR);
        assert_eq!(replies[1]["error"]["code"], PARSE_ERROR);
        assert_eq!(replies[2]["id"], 3);
        assert!(replies[2]["result"].is_object());
    }

    #[test]
    fn unsupported_per_request_version_is_rejected() {
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2000-01-01"}}}"#;
        let r = handle_line(line).unwrap();
        assert_eq!(r["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(r["error"]["data"]["supported"][0], "2026-07-28");
    }
}
