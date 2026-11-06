// SPDX-License-Identifier: GPL-3.0-or-later
//! MCP server for the shellcheck-rust linter.
//!
//! The transport is stdio: one JSON-RPC 2.0 message per line. The protocol
//! layer is hand-written (about 150 lines) because the surface is small:
//! initialize / server/discover, ping, tools/list and tools/call. See
//! docs/mcp.md for the client configuration.

pub mod catalog;
pub mod tools;

use std::io::{self, BufRead, Write};

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

/// Read JSON-RPC messages from `input` and write responses to `output` until
/// `input` ends. Notifications get no response.
pub fn serve<R: BufRead, W: Write>(input: R, mut output: W) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(&line) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Handle one raw line. Returns the reply to send, if any.
pub fn handle_line(line: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(line) {
        Ok(msg) => handle_message(&msg),
        Err(e) => Some(error_reply(Value::Null, RpcError::new(PARSE_ERROR, format!("Parse error: {e}")))),
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
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    Ok(tools::call(name, &args).to_json())
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
        let r = handle_line(r#"{"jsonrpc":"2.0","id":7,"method":"nope"}"#).unwrap();
        assert_eq!((r["id"].clone(), r["error"]["code"].clone()), (json!(7), json!(METHOD_NOT_FOUND)));
    }

    #[test]
    fn unsupported_per_request_version_is_rejected() {
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2000-01-01"}}}"#;
        let r = handle_line(line).unwrap();
        assert_eq!(r["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(r["error"]["data"]["supported"][0], "2026-07-28");
    }
}
