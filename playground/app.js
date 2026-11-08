// SPDX-License-Identifier: GPL-3.0-or-later
// ShellCheck playground: a textarea over a highlighted backdrop, live lint
// through the wasm module, diagnostics with wiki links, fixes, and a
// base64 script in the URL hash.
import init, { check, fix, list_checks, version } from "./pkg/shelllint_wasm.js";

const WIKI = "https://www.shellcheck.net/wiki/";
const DEBOUNCE_MS = 150;
const RANK = { error: 0, warning: 1, info: 2, style: 3 };
const SAMPLE = `#!/bin/bash
dir=$1
files=$(ls $dir)
for f in $files; do
  echo $f
done
[ $dir == "/tmp" ] && echo "tmp"
cd "$dir"
rm -rf $HOME/*
`;

const $ = (id) => document.getElementById(id);
const el = {
  src: $("src"), backdrop: $("backdrop"), gutter: $("gutter"), diags: $("diags"),
  status: $("status"), shell: $("shell"), severity: $("severity"), fixAll: $("fixAll"),
  share: $("share"), optional: $("optional"), ver: $("ver"),
};

let comments = [];
let timer = 0;

// ---- state in the URL hash -------------------------------------------------

function toBase64Url(text) {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function fromBase64Url(s) {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(b64.padEnd(b64.length + ((4 - (b64.length % 4)) % 4), "="));
  return new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
}

function readHash() {
  try {
    const p = new URLSearchParams(location.hash.slice(1));
    return { script: p.has("s") ? fromBase64Url(p.get("s")) : null, shell: p.get("shell") };
  } catch {
    return { script: null, shell: null };
  }
}

function writeHash() {
  const params = new URLSearchParams();
  params.set("s", toBase64Url(el.src.value));
  if (el.shell.value !== "auto") params.set("shell", el.shell.value);
  history.replaceState(null, "", "#" + params.toString());
}

// ---- options ----------------------------------------------------------------

function options() {
  const enable = [...el.optional.querySelectorAll("input:checked")].map((i) => i.value);
  return JSON.stringify({
    shell: el.shell.value === "auto" ? "" : el.shell.value,
    severity: el.severity.value,
    enable,
  });
}

// ---- lint and render --------------------------------------------------------

function lint() {
  const script = el.src.value;
  let error = null;
  try {
    comments = JSON.parse(check(script, options())).comments;
  } catch (e) {
    comments = [];
    error = String(e);
  }
  render(script, error);
  writeHash();
}

function scheduleLint() {
  clearTimeout(timer);
  timer = setTimeout(lint, DEBOUNCE_MS);
}

function render(script, error) {
  const lines = script.split("\n").map((l) => Array.from(l));
  const ranks = mark(lines);
  renderBackdrop(lines, ranks);
  renderGutter(lines.length);
  renderDiagnostics();
  el.fixAll.disabled = !comments.some((c) => c.fix);
  if (error) {
    el.status.textContent = error;
  } else if (comments.length === 0) {
    el.status.textContent = "No issues";
  } else {
    el.status.textContent = `${comments.length} issue${comments.length === 1 ? "" : "s"}`;
  }
}

/// Per character: the rank of the most severe diagnostic covering it
/// (Infinity if none). A range is [column, endColumn) on its lines, at least
/// one character wide.
function mark(lines) {
  const ranks = lines.map((l) => new Array(l.length).fill(Infinity));
  comments.forEach((c) => {
    const rank = RANK[c.level] ?? 3;
    for (let ln = c.line; ln <= c.endLine; ln++) {
      const chars = lines[ln - 1];
      if (!chars) continue;
      const start = ln === c.line ? c.column : 1;
      let end = ln === c.endLine ? c.endColumn : chars.length + 1;
      if (end <= start) end = start + 1;
      for (let col = start; col < end && col - 1 < chars.length; col++) {
        const i = col - 1;
        if (rank < ranks[ln - 1][i]) ranks[ln - 1][i] = rank;
      }
    }
  });
  return ranks;
}

const LEVEL_OF_RANK = ["error", "warning", "info", "style"];

function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]));
}

function renderBackdrop(lines, ranks) {
  // Runs of equal rank become one span; the text itself is kept verbatim so
  // the backdrop lines up with the textarea.
  const html = lines
    .map((chars, ln) => {
      let out = "";
      let i = 0;
      while (i < chars.length) {
        const r = ranks[ln][i];
        let j = i + 1;
        while (j < chars.length && ranks[ln][j] === r) j++;
        const text = escapeHtml(chars.slice(i, j).join(""));
        out += r === Infinity ? text : `<span class="m sev-${LEVEL_OF_RANK[r]}">${text}</span>`;
        i = j;
      }
      return out;
    })
    .join("\n");
  el.backdrop.innerHTML = html + "\n ";
}

function renderGutter(count) {
  const worst = new Array(count).fill(null);
  for (const c of comments) {
    for (let ln = c.line; ln <= Math.min(c.endLine, count); ln++) {
      const cur = worst[ln - 1];
      if (cur === null || RANK[c.level] < RANK[cur]) worst[ln - 1] = c.level;
    }
  }
  const frag = document.createDocumentFragment();
  for (let i = 0; i < count; i++) {
    const d = document.createElement("div");
    d.className = "ln" + (worst[i] ? ` sev-${worst[i]}` : "");
    d.textContent = String(i + 1);
    frag.appendChild(d);
  }
  el.gutter.replaceChildren(frag);
}

function renderDiagnostics() {
  if (comments.length === 0) {
    const p = document.createElement("li");
    p.className = "empty";
    p.textContent = "Nothing to report.";
    el.diags.replaceChildren(p);
    return;
  }
  const items = comments.map((c, idx) => {
    const li = document.createElement("li");
    li.className = `diag sev-${c.level}`;

    const jump = document.createElement("button");
    jump.type = "button";
    jump.className = "jump";
    jump.dataset.index = String(idx);
    jump.setAttribute("aria-label", `Go to line ${c.line}, column ${c.column}`);

    const head = document.createElement("div");
    head.className = "head";
    const where = document.createElement("span");
    where.textContent = `${c.line}:${c.column} ${c.level}`;
    head.appendChild(where);
    const code = document.createElement("a");
    code.href = `${WIKI}SC${c.code}`;
    code.target = "_blank";
    code.rel = "noopener";
    code.textContent = `SC${c.code}`;
    code.addEventListener("click", (e) => e.stopPropagation());
    head.appendChild(code);

    const msg = document.createElement("div");
    msg.className = "msg";
    msg.textContent = c.message;

    jump.append(head, msg);
    li.appendChild(jump);

    if (c.fix) {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "fix";
      b.dataset.fix = String(idx);
      b.textContent = "Apply fix";
      li.appendChild(b);
    }
    return li;
  });
  el.diags.replaceChildren(...items);
}

function jumpTo(c) {
  const lines = el.src.value.split("\n");
  let offset = 0;
  for (let i = 0; i < c.line - 1 && i < lines.length; i++) offset += lines[i].length + 1;
  const line = lines[c.line - 1] ?? "";
  // Columns count code points; convert to UTF-16 offsets.
  const col = Array.from(line).slice(0, c.column - 1).join("").length;
  const pos = offset + col;
  el.src.focus();
  el.src.setSelectionRange(pos, pos);
  const lineHeight = 21;
  el.src.scrollTop = Math.max(0, (c.line - 3) * lineHeight);
  syncScroll();
}

// ---- fixes ------------------------------------------------------------------

function applyFix(index) {
  try {
    const out = fix(el.src.value, options(), index);
    el.src.value = out;
    lint();
    el.status.textContent = index < 0 ? "All fixes applied" : "Fix applied";
  } catch (e) {
    el.status.textContent = String(e);
  }
}

// ---- scroll sync -------------------------------------------------------------

function syncScroll() {
  el.backdrop.scrollTop = el.src.scrollTop;
  el.backdrop.scrollLeft = el.src.scrollLeft;
  el.gutter.scrollTop = el.src.scrollTop;
}

// ---- setup -------------------------------------------------------------------

function renderOptionalChecks() {
  let list = [];
  try {
    list = JSON.parse(list_checks());
  } catch {
    list = [];
  }
  const nodes = list.map((c) => {
    const label = document.createElement("label");
    const input = document.createElement("input");
    input.type = "checkbox";
    input.value = c.name;
    input.addEventListener("change", scheduleLint);
    const text = document.createElement("span");
    text.textContent = c.name;
    const small = document.createElement("small");
    small.textContent = c.description;
    const wrap = document.createElement("span");
    wrap.append(text, small);
    label.append(input, wrap);
    return label;
  });
  el.optional.replaceChildren(...nodes);
}

async function main() {
  el.status.textContent = "Loading...";
  await init();
  el.ver.textContent = `· port of ShellCheck v${version()}`;

  const hash = readHash();
  el.src.value = hash.script ?? SAMPLE;
  if (hash.shell && [...el.shell.options].some((o) => o.value === hash.shell)) {
    el.shell.value = hash.shell;
  }

  renderOptionalChecks();
  el.src.addEventListener("input", scheduleLint);
  el.src.addEventListener("scroll", syncScroll);
  el.shell.addEventListener("change", lint);
  el.severity.addEventListener("change", lint);

  el.diags.addEventListener("click", (e) => {
    const fixBtn = e.target.closest("button.fix");
    if (fixBtn) return applyFix(Number(fixBtn.dataset.fix));
    const jump = e.target.closest("button.jump");
    if (jump) jumpTo(comments[Number(jump.dataset.index)]);
  });
  el.fixAll.addEventListener("click", () => applyFix(-1));
  el.share.addEventListener("click", async () => {
    writeHash();
    try {
      await navigator.clipboard.writeText(location.href);
      el.status.textContent = "Link copied";
    } catch {
      el.status.textContent = "Copy the address bar link";
    }
  });
  window.addEventListener("hashchange", () => {
    const h = readHash();
    if (h.script !== null && h.script !== el.src.value) {
      el.src.value = h.script;
      lint();
    }
  });

  lint();
}

main().catch((e) => {
  el.status.textContent = `Failed to load WebAssembly: ${e}`;
});
