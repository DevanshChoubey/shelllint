// SPDX-License-Identifier: GPL-3.0-or-later
//! LSP framing over stdio that survives malformed input.
//!
//! `lsp_server::Connection::stdio` stops serving at the first bad frame. Here
//! a bad frame (bad header, invalid UTF-8, bad JSON, a JSON array) is answered
//! with a JSON-RPC error carrying a null id and reading continues; only end of
//! input ends the session.

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Sender, bounded};
use lsp_server::{Connection, Message};
use serde_json::{Value, json};

/// Largest accepted `Content-Length`; anything bigger is refused, not allocated.
const MAX_BODY: usize = 256 * 1024 * 1024;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;

type SharedOut = Arc<Mutex<Box<dyn Write + Send>>>;

enum Frame {
    Eof,
    Body(Vec<u8>),
    /// A frame that cannot be used, with the reason for the error reply.
    Bad(String),
}

fn read_frame(r: &mut impl BufRead) -> io::Result<Frame> {
    let mut len: Option<usize> = None;
    let mut bad: Option<String> = None;
    loop {
        let mut raw = Vec::new();
        if r.read_until(b'\n', &mut raw)? == 0 {
            // End of input; a half-read header section ends the session too.
            return Ok(Frame::Eof);
        }
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        match line.split_once(':') {
            Some((name, value)) if name.trim().eq_ignore_ascii_case("content-length") => {
                match value.trim().parse::<usize>() {
                    Ok(n) if n <= MAX_BODY => len = Some(n),
                    Ok(n) => bad = Some(format!("Content-Length {n} is too large")),
                    Err(_) => bad = Some(format!("invalid Content-Length: {}", value.trim())),
                }
            }
            Some((name, _)) if name.trim().eq_ignore_ascii_case("content-type") => {}
            _ => bad = Some(format!("malformed header: {}", line.chars().take(60).collect::<String>())),
        }
    }
    if let Some(m) = bad {
        return Ok(Frame::Bad(m));
    }
    let Some(n) = len else {
        return Ok(Frame::Bad("missing Content-Length header".to_string()));
    };
    let mut body = vec![0u8; n];
    r.read_exact(&mut body)?;
    Ok(Frame::Body(body))
}

enum Parsed {
    Msg(Message),
    Bad(i64, String),
}

fn parse_body(body: &[u8]) -> Parsed {
    let Ok(text) = std::str::from_utf8(body) else {
        return Parsed::Bad(PARSE_ERROR, "Parse error: body is not valid UTF-8".to_string());
    };
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return Parsed::Bad(PARSE_ERROR, format!("Parse error: {e}")),
    };
    if value.is_array() {
        return Parsed::Bad(PARSE_ERROR, "Parse error: batches are not supported, expected a JSON object".to_string());
    }
    match serde_json::from_value::<Message>(value) {
        Ok(m) => Parsed::Msg(m),
        Err(e) => Parsed::Bad(INVALID_REQUEST, format!("Invalid Request: {e}")),
    }
}

fn write_raw(out: &SharedOut, body: &str) -> io::Result<()> {
    let mut w = out.lock().unwrap_or_else(|p| p.into_inner());
    write!(w, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    w.flush()
}

fn send_error(out: &SharedOut, code: i64, message: &str) -> io::Result<()> {
    let body = json!({"jsonrpc": "2.0", "id": null, "error": {"code": code, "message": message}});
    write_raw(out, &body.to_string())
}

fn reader_loop(mut input: impl BufRead, to_server: Sender<Message>, out: SharedOut) {
    loop {
        let reply = match read_frame(&mut input) {
            Ok(Frame::Eof) | Err(_) => return,
            Ok(Frame::Bad(m)) => (PARSE_ERROR, format!("Parse error: {m}")),
            Ok(Frame::Body(b)) => match parse_body(&b) {
                Parsed::Msg(m) => {
                    if to_server.send(m).is_err() {
                        return;
                    }
                    continue;
                }
                Parsed::Bad(code, m) => (code, m),
            },
        };
        if send_error(&out, reply.0, &reply.1).is_err() {
            return;
        }
    }
}

/// A connection reading `input` and writing `output`. The returned handle is
/// the writer thread; it ends once the connection (and its clones) are dropped.
pub fn connection(
    input: impl BufRead + Send + 'static,
    output: impl Write + Send + 'static,
) -> (Connection, JoinHandle<()>) {
    let out: SharedOut = Arc::new(Mutex::new(Box::new(output)));
    let (to_server, receiver) = bounded::<Message>(0);
    let (sender, from_server) = bounded::<Message>(0);
    let reader_out = out.clone();
    thread::spawn(move || reader_loop(input, to_server, reader_out));
    let writer = thread::spawn(move || {
        for msg in from_server {
            let Ok(body) = serde_json::to_string(&msg) else { continue };
            if write_raw(&out, &body).is_err() {
                break;
            }
        }
    });
    (Connection { sender, receiver }, writer)
}
