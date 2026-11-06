// SPDX-License-Identifier: GPL-3.0-or-later
//! The LSP main loop. Requests and notifications are handled on the main
//! thread; checks run on one worker thread and come back over a channel, so a
//! slow check never blocks request handling. Edits are debounced; open and
//! save check immediately. Results for an out-of-date version are dropped.

use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{CodeActionRequest, HoverRequest, Request as _};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionProviderCapability,
    CodeDescription, Diagnostic, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, MarkupContent, MarkupKind, NumberOrString,
    Position, PublishDiagnosticsParams, Range, SaveOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextDocumentSyncSaveOptions, TextEdit, Url, WorkspaceEdit,
};

use crate::check::{Finding, WIKI_LINK, disable_edits, run_check};
use crate::config::Settings;
use crate::document::Document;
use crate::position::split_lines;

/// Quiet period after the last edit before a check runs.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

type BoxErr = Box<dyn Error + Sync + Send>;

struct Job {
    uri: Url,
    version: i32,
    text: String,
    filename: String,
    ignore_rc: bool,
    settings: Settings,
}

struct Done {
    uri: Url,
    version: i32,
    findings: Vec<Finding>,
}

struct Cached {
    version: i32,
    findings: Vec<Finding>,
}

pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::INCREMENTAL),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions { include_text: Some(false) })),
            ..Default::default()
        })),
        code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..Default::default()
    }
}

/// Serve LSP on stdin/stdout until the client shuts down.
pub fn run_stdio() -> Result<(), BoxErr> {
    let (connection, io_threads) = Connection::stdio();
    serve(connection)?;
    io_threads.join()?;
    Ok(())
}

/// Serve LSP over an existing connection (used by the tests as well).
pub fn serve(connection: Connection) -> Result<(), BoxErr> {
    let init = connection.initialize(serde_json::to_value(server_capabilities())?)?;
    let params: InitializeParams = serde_json::from_value(init)?;
    let settings = params.initialization_options.as_ref().map(Settings::from_json).unwrap_or_default();

    let (job_tx, job_rx) = unbounded::<Job>();
    let (done_tx, done_rx) = unbounded::<Done>();
    thread::spawn(move || worker(job_rx, done_tx));

    let mut srv = Server {
        sender: connection.sender.clone(),
        jobs: job_tx,
        docs: HashMap::new(),
        cache: HashMap::new(),
        pending: HashMap::new(),
        settings,
    };
    loop {
        let timeout = srv.next_timeout();
        select! {
            recv(connection.receiver) -> msg => match msg {
                Ok(Message::Request(req)) => {
                    if connection.handle_shutdown(&req)? {
                        break;
                    }
                    srv.on_request(req);
                }
                Ok(Message::Notification(n)) => srv.on_notification(n),
                Ok(Message::Response(_)) => {}
                Err(_) => break,
            },
            recv(done_rx) -> done => {
                if let Ok(d) = done {
                    srv.on_done(d);
                }
            }
            default(timeout) => srv.flush_due(),
        }
    }
    Ok(())
}

fn worker(jobs: Receiver<Job>, out: Sender<Done>) {
    for j in jobs {
        let findings = run_check(&j.text, &j.filename, j.ignore_rc, &j.settings);
        if out.send(Done { uri: j.uri, version: j.version, findings }).is_err() {
            break;
        }
    }
}

struct Server {
    sender: Sender<Message>,
    jobs: Sender<Job>,
    docs: HashMap<Url, Document>,
    cache: HashMap<Url, Cached>,
    pending: HashMap<Url, Instant>,
    settings: Settings,
}

impl Server {
    fn next_timeout(&self) -> Duration {
        let now = Instant::now();
        self.pending
            .values()
            .min()
            .map(|t| t.saturating_duration_since(now))
            .unwrap_or(Duration::from_secs(3600))
    }

    fn flush_due(&mut self) {
        let now = Instant::now();
        let due: Vec<Url> = self.pending.iter().filter(|(_, t)| **t <= now).map(|(u, _)| u.clone()).collect();
        for u in due {
            self.pending.remove(&u);
            self.submit(&u);
        }
    }

    fn submit(&self, uri: &Url) {
        if let Some(doc) = self.docs.get(uri) {
            let _ = self.jobs.send(Job {
                uri: uri.clone(),
                version: doc.version,
                text: doc.text.clone(),
                filename: doc.filename.clone(),
                ignore_rc: !doc.is_file,
                settings: self.settings.clone(),
            });
        }
    }

    fn publish(&self, uri: Url, diagnostics: Vec<Diagnostic>, version: Option<i32>) {
        let params = PublishDiagnosticsParams { uri, diagnostics, version };
        let _ = self.sender.send(Message::Notification(Notification::new(
            PublishDiagnostics::METHOD.to_string(),
            params,
        )));
    }

    fn respond<T: serde::Serialize>(&self, id: RequestId, result: T) {
        let _ = self.sender.send(Message::Response(Response::new_ok(id, result)));
    }

    fn respond_err(&self, id: RequestId, msg: String) {
        let _ = self.sender.send(Message::Response(Response::new_err(id, ErrorCode::InvalidParams as i32, msg)));
    }

    fn on_done(&mut self, d: Done) {
        let current = matches!(self.docs.get(&d.uri), Some(doc) if doc.version == d.version);
        if !current {
            return;
        }
        let diags = d.findings.iter().map(|f| to_diagnostic(f, &d.uri)).collect();
        self.publish(d.uri.clone(), diags, Some(d.version));
        self.cache.insert(d.uri, Cached { version: d.version, findings: d.findings });
    }

    /// Findings for the document's current version: cached when the worker
    /// has already checked it, otherwise checked now.
    fn findings(&mut self, uri: &Url) -> Vec<Finding> {
        let Some(doc) = self.docs.get(uri) else { return vec![] };
        let version = doc.version;
        if let Some(c) = self.cache.get(uri) {
            if c.version == version {
                return c.findings.clone();
            }
        }
        let findings = run_check(&doc.text, &doc.filename, !doc.is_file, &self.settings);
        self.cache.insert(uri.clone(), Cached { version, findings: findings.clone() });
        findings
    }

    fn on_request(&mut self, req: Request) {
        let id = req.id.clone();
        match req.method.as_str() {
            HoverRequest::METHOD => match serde_json::from_value::<HoverParams>(req.params) {
                Ok(p) => {
                    let uri = p.text_document_position_params.text_document.uri.clone();
                    let pos = p.text_document_position_params.position;
                    let hover = self.hover(&uri, pos);
                    self.respond(id, hover);
                }
                Err(e) => self.respond_err(id, e.to_string()),
            },
            CodeActionRequest::METHOD => match serde_json::from_value::<CodeActionParams>(req.params) {
                Ok(p) => {
                    let actions = self.code_actions(&p.text_document.uri, p.range);
                    self.respond(id, actions);
                }
                Err(e) => self.respond_err(id, e.to_string()),
            },
            other => {
                let _ = self.sender.send(Message::Response(Response::new_err(
                    id,
                    ErrorCode::MethodNotFound as i32,
                    format!("unsupported request: {}", other),
                )));
            }
        }
    }

    /// Hover lists every finding under the position, one block each.
    fn hover(&mut self, uri: &Url, pos: Position) -> Option<Hover> {
        let findings = self.findings(uri);
        let blocks: Vec<String> = findings
            .iter()
            .filter(|f| f.contains(pos))
            .map(|f| {
                let mut b = format!(
                    "**SC{}** ({})\n\n{}\n\n[Wiki: SC{}]({}SC{})",
                    f.code,
                    severity_name(f),
                    f.message,
                    f.code,
                    WIKI_LINK,
                    f.code
                );
                if !f.fix.is_empty() {
                    b.push_str("\n\n_A quick fix is available._");
                }
                b
            })
            .collect();
        if blocks.is_empty() {
            return None;
        }
        let range = findings.iter().find(|f| f.contains(pos)).map(|f| f.range);
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: blocks.join("\n\n---\n\n"),
            }),
            range,
        })
    }

    fn code_actions(&mut self, uri: &Url, range: Range) -> Vec<CodeActionOrCommand> {
        let findings = self.findings(uri);
        let Some(doc) = self.docs.get(uri) else { return vec![] };
        let lines = split_lines(&doc.text);
        let mut out = vec![];
        let mut disabled: Vec<(u32, i64)> = vec![];
        for f in findings.iter().filter(|f| f.range.start <= range.end && range.start <= f.range.end) {
            let diag = to_diagnostic(f, uri);
            if !f.fix.is_empty() {
                let mut changes = HashMap::new();
                changes.insert(uri.clone(), f.fix.clone());
                out.push(action(
                    format!("Apply ShellCheck fix for SC{}", f.code),
                    Some(diag.clone()),
                    WorkspaceEdit { changes: Some(changes), ..Default::default() },
                    true,
                ));
            }
            let key = (f.range.start.line, f.code);
            if disabled.contains(&key) {
                continue;
            }
            disabled.push(key);
            let edits: Vec<TextEdit> = disable_edits(&lines, f.range.start.line, f.code);
            if !edits.is_empty() {
                let mut changes = HashMap::new();
                changes.insert(uri.clone(), edits);
                out.push(action(
                    format!("Disable SC{} for this line", f.code),
                    Some(diag),
                    WorkspaceEdit { changes: Some(changes), ..Default::default() },
                    false,
                ));
            }
        }
        out
    }

    fn on_notification(&mut self, n: Notification) {
        match n.method.as_str() {
            DidOpenTextDocument::METHOD => {
                if let Ok(p) = serde_json::from_value::<DidOpenTextDocumentParams>(n.params) {
                    let td = p.text_document;
                    let doc = new_document(td.uri.clone(), td.version, td.text);
                    self.docs.insert(td.uri.clone(), doc);
                    self.pending.remove(&td.uri);
                    self.submit(&td.uri);
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Ok(p) = serde_json::from_value::<DidChangeTextDocumentParams>(n.params) {
                    let uri = p.text_document.uri;
                    if let Some(doc) = self.docs.get_mut(&uri) {
                        for c in &p.content_changes {
                            doc.apply(c);
                        }
                        doc.version = p.text_document.version;
                        self.pending.insert(uri, Instant::now() + DEBOUNCE);
                    }
                }
            }
            DidSaveTextDocument::METHOD => {
                if let Ok(p) = serde_json::from_value::<DidSaveTextDocumentParams>(n.params) {
                    self.pending.remove(&p.text_document.uri);
                    self.submit(&p.text_document.uri);
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Ok(p) = serde_json::from_value::<DidCloseTextDocumentParams>(n.params) {
                    let uri = p.text_document.uri;
                    self.docs.remove(&uri);
                    self.cache.remove(&uri);
                    self.pending.remove(&uri);
                    self.publish(uri, vec![], None);
                }
            }
            DidChangeConfiguration::METHOD => {
                if let Ok(p) = serde_json::from_value::<DidChangeConfigurationParams>(n.params) {
                    if p.settings.is_null() {
                        return;
                    }
                    let new = Settings::from_json(&p.settings);
                    if new != self.settings {
                        self.settings = new;
                        self.cache.clear();
                        let uris: Vec<Url> = self.docs.keys().cloned().collect();
                        for u in uris {
                            self.pending.remove(&u);
                            self.submit(&u);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn new_document(uri: Url, version: i32, text: String) -> Document {
    let is_file = uri.scheme() == "file";
    let filename = if is_file {
        uri.to_file_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| uri.path().to_string())
    } else {
        uri.path_segments().and_then(|mut s| s.next_back()).filter(|s| !s.is_empty()).unwrap_or("untitled.sh").to_string()
    };
    Document { text, version, filename, is_file }
}

fn severity_name(f: &Finding) -> &'static str {
    use lsp_types::DiagnosticSeverity as D;
    match f.severity {
        D::ERROR => "error",
        D::WARNING => "warning",
        D::INFORMATION => "info",
        _ => "style",
    }
}

fn to_diagnostic(f: &Finding, uri: &Url) -> Diagnostic {
    let _ = uri;
    Diagnostic {
        range: f.range,
        severity: Some(f.severity),
        code: Some(NumberOrString::String(format!("SC{}", f.code))),
        code_description: Url::parse(&format!("{}SC{}", WIKI_LINK, f.code)).ok().map(|href| CodeDescription { href }),
        source: Some("shellcheck".to_string()),
        message: f.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

fn action(title: String, diagnostic: Option<Diagnostic>, edit: WorkspaceEdit, preferred: bool) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: diagnostic.map(|d| vec![d]),
        edit: Some(edit),
        command: None,
        is_preferred: Some(preferred),
        disabled: None,
        data: None,
    })
}
