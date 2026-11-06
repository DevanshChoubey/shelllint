// SPDX-License-Identifier: GPL-3.0-or-later
(function () {
  "use strict";

  const SVG_NS = "http://www.w3.org/2000/svg";
  const CODE_PAGE = 60;
  let codeRows = [];
  let showAllCodes = false;

  // ---- helpers -------------------------------------------------------------
  function $(id) { return document.getElementById(id); }

  function el(tag, props, children) {
    const n = document.createElement(tag);
    if (props) for (const k of Object.keys(props)) {
      if (k === "class") n.className = props[k];
      else if (k === "text") n.textContent = props[k];
      else n.setAttribute(k, props[k]);
    }
    for (const c of children || []) if (c) n.appendChild(c);
    return n;
  }

  function svg(tag, attrs) {
    const n = document.createElementNS(SVG_NS, tag);
    for (const k of Object.keys(attrs || {})) n.setAttribute(k, attrs[k]);
    return n;
  }

  function svgText(x, y, text, cls) {
    const t = svg("text", { x: x, y: y, class: cls || "" });
    t.textContent = text;
    return t;
  }

  function fmtPct(v, digits) {
    return v == null ? "-" : v.toFixed(digits == null ? 1 : digits) + "%";
  }
  function fmtNum(v) { return v == null ? "-" : v.toLocaleString("en-US"); }
  function fmtSec(v) { return v == null ? "-" : v < 10 ? v.toFixed(2) + " s" : v.toFixed(1) + " s"; }
  function fmtMb(v) { return v == null ? "-" : v.toFixed(1) + " MB"; }
  function geomean(xs) {
    const v = xs.filter(function (x) { return x && x > 0; });
    if (!v.length) return null;
    return Math.exp(v.reduce(function (a, x) { return a + Math.log(x); }, 0) / v.length);
  }

  function showNote(msg) {
    const box = $("notes");
    box.hidden = false;
    box.appendChild(el("p", { text: msg }));
  }

  // ---- theme ---------------------------------------------------------------
  const THEMES = ["auto", "light", "dark"];
  function readTheme() {
    try { return localStorage.getItem("sc-theme") || "auto"; } catch (e) { return "auto"; }
  }
  function applyTheme(t) {
    if (t === "auto") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", t);
    $("theme-label").textContent = t[0].toUpperCase() + t.slice(1);
    $("theme-toggle").setAttribute("aria-label", "Color theme: " + t + ". Activate to change.");
  }
  function setupTheme() {
    let t = readTheme();
    applyTheme(t);
    $("theme-toggle").addEventListener("click", function () {
      t = THEMES[(THEMES.indexOf(t) + 1) % THEMES.length];
      applyTheme(t);
      try { localStorage.setItem("sc-theme", t); } catch (e) { /* private mode: keep in-memory only */ }
    });
  }

  // ---- stats ---------------------------------------------------------------
  function renderStats(d) {
    const c = d.conformance, r = d.real;
    if (c && c.pass_pct != null) {
      $("stat-pass").textContent = fmtPct(c.pass_pct);
      $("stat-pass-sub").textContent = fmtNum(c.pass) + " of " + fmtNum(c.total) + " tests";
    }
    if (r) {
      $("stat-agree").textContent = fmtPct(r.totals.files_identical_pct);
      $("stat-agree-sub").textContent = fmtNum(r.totals.files_identical) + " of " + fmtNum(r.totals.files_compared) + " files identical";
      const speeds = r.repos.map(function (x) { return x.speedup; });
      const g = geomean(speeds);
      $("stat-speed").textContent = g ? g.toFixed(1) + "x" : "-";
      $("stat-speed-sub").textContent = "geometric mean over " + r.repos.length + " repos";
      const mems = r.repos.map(function (x) { return x.hs_peak_rss_mb && x.rs_peak_rss_mb ? x.hs_peak_rss_mb / x.rs_peak_rss_mb : null; });
      const gm = geomean(mems);
      $("stat-mem").textContent = gm ? gm.toFixed(1) + "x less" : "-";
      $("stat-mem-sub").textContent = "Haskell / Rust, geometric mean of peaks";
    }
  }

  // ---- conformance ---------------------------------------------------------
  function renderConformance(d) {
    const c = d.conformance;
    const bar = $("conf-bar"), legend = $("conf-legend");
    if (c.pass == null) {
      bar.hidden = true;
      legend.hidden = true;
      $("conf-fails").hidden = true;
      $("conformance").appendChild(el("p", { class: "muted", text: "No conformance report yet." }));
      return;
    }
    const parts = [
      ["pass", "Pass", c.pass, "seg-pass"],
      ["fail", "Fail", c.fail, "seg-fail"],
      ["blocked", "Blocked", c.blocked, "seg-blocked"],
      ["unported", "Unported", c.unported, "seg-unported"],
    ];
    parts.forEach(function (p) {
      const pct = c.total ? (100 * p[2]) / c.total : 0;
      if (pct > 0) bar.appendChild(el("span", { class: p[3], style: "width:" + pct + "%", title: p[1] + ": " + p[2] }));
      legend.appendChild(el("li", null, [
        el("span", { class: "swatch " + p[3] }),
        document.createTextNode(p[1] + " " + fmtNum(p[2])),
      ]));
    });
    const fl = $("conf-fail-list");
    const fails = d.conformance_failures || [];
    if (!fails.length) fl.appendChild(el("li", { text: "No failing tests." }));
    fails.slice(0, 500).forEach(function (f) {
      fl.appendChild(el("li", { text: f.key + ": " + f.message }));
    });
    if (fails.length > 500) fl.appendChild(el("li", { text: "... " + (fails.length - 500) + " more" }));
    $("conf-fails").querySelector("summary").textContent = "Failing tests (" + fails.length + ")";
  }

  // ---- per-code table ------------------------------------------------------
  function statusOf(row) {
    if (row.failing > 0) return ["failing", "b-fail", "failing"];
    if (row.tests > 0) return ["passing", "b-pass", "passing"];
    if (row.registered) return ["no tests", "b-untested", "no tests"];
    return ["unregistered", "b-none", "unregistered"];
  }

  function renderCodeTable() {
    const q = $("code-search").value.trim().toLowerCase();
    const f = $("code-filter").value;
    const rows = codeRows.filter(function (r) {
      if (q && String(r.code).indexOf(q.replace(/^sc/, "")) === -1) return false;
      if (f === "tested") return r.tests > 0;
      if (f === "untested") return r.registered && r.tests === 0;
      if (f === "failing") return r.failing > 0;
      return true;
    });
    const body = $("code-body");
    body.textContent = "";
    const limit = showAllCodes ? rows.length : CODE_PAGE;
    rows.slice(0, limit).forEach(function (r) {
      const st = statusOf(r);
      body.appendChild(el("tr", null, [
        el("td", null, [el("a", { href: "https://www.shellcheck.net/wiki/SC" + r.code, target: "_blank", rel: "noopener", class: "mono", text: "SC" + r.code })]),
        el("td", { class: "num", text: fmtNum(r.tests) }),
        el("td", { class: "num", text: fmtNum(r.failing) }),
        el("td", null, [el("span", { class: "badge " + st[1], text: st[2] })]),
        el("td", { class: "mods", text: r.modules.join(", ") }),
      ]));
    });
    $("code-summary").textContent = "Showing " + Math.min(limit, rows.length) + " of " + rows.length + " codes.";
    $("code-more").hidden = rows.length <= CODE_PAGE;
    $("code-more").textContent = showAllCodes ? "Show fewer" : "Show all " + rows.length;
  }

  function renderCorpusCodes(d) {
    const cp = d.corpus;
    codeRows = cp.per_code;
    ["code-search", "code-filter"].forEach(function (id) {
      $(id).addEventListener("input", function () { showAllCodes = false; renderCodeTable(); });
    });
    $("code-filter").addEventListener("change", function () { showAllCodes = false; renderCodeTable(); });
    $("code-more").addEventListener("click", function () { showAllCodes = !showAllCodes; renderCodeTable(); });
    renderCodeTable();
    $("code-summary").textContent += "  " + cp.codes_with_tests + " codes have tests; " +
      cp.codes_registered_untested + " registered codes have none.";
  }

  // ---- real repos ----------------------------------------------------------
  function renderReal(d) {
    const r = d.real;
    if (!r) {
      $("real").appendChild(el("p", { class: "muted", text: "No real-repo run yet." }));
      ["speed", "memory", "mismatch"].forEach(function (id) { $(id).hidden = true; });
      return;
    }
    const body = $("real-body");
    r.repos.forEach(function (x) {
      const pct = x.identical_pct;
      const bar = el("span", { class: "pct-bar" }, [el("span", { style: "width:" + (pct || 0) + "%" })]);
      body.appendChild(el("tr", null, [
        el("td", null, [el("strong", { text: x.name })]),
        el("td", { class: "num", text: fmtNum(x.files) }),
        el("td", { class: "num", text: fmtNum(x.lines) }),
        el("td", { class: "num" }, [
          document.createTextNode(fmtPct(pct) + " "),
          bar,
        ]),
        el("td", { class: "num", text: fmtNum(x.files_compared - x.files_identical) }),
      ]));
    });
  }

  // ---- charts --------------------------------------------------------------
  function barChart(container, repos, hsKey, rsKey, unit, fmt) {
    const rowH = 58, leftPad = 0, w = 640, barX = 0, barMax = w - 110, labelH = 20;
    const max = Math.max.apply(null, repos.flatMap(function (x) { return [x[hsKey] || 0, x[rsKey] || 0]; })) || 1;
    const h = repos.length * rowH + 12;
    const s = svg("svg", { viewBox: "0 0 " + w + " " + h, role: "img",
      "aria-label": unit + " per repo, Haskell versus Rust" });
    repos.forEach(function (x, i) {
      const y = i * rowH + 6;
      s.appendChild(svgText(leftPad, y + labelH - 4, x.name));
      s.appendChild(svgText(w, y + labelH - 4, x.files + " files", "sub")).setAttribute("text-anchor", "end");
      [[hsKey, "hs", y + labelH + 2], [rsKey, "rs", y + labelH + 20]].forEach(function (b) {
        const v = x[b[0]] || 0;
        const len = Math.max(2, (v / max) * barMax);
        s.appendChild(svg("rect", { x: barX, y: b[2], width: len, height: 12, rx: 3, class: "bar " + b[1] }));
        const t = svgText(barX + len + 6, b[2] + 11, fmt(x[b[0]]), "val");
        s.appendChild(t);
      });
    });
    container.textContent = "";
    container.appendChild(s);
  }

  function renderCharts(d) {
    const r = d.real;
    if (!r) return;
    barChart($("speed-chart"), r.repos, "hs_wall_s", "rs_wall_s", "Wall time", fmtSec);
    const mem = r.repos.filter(function (x) { return x.hs_peak_rss_mb && x.rs_peak_rss_mb; });
    barChart($("memory-chart"), mem, "hs_peak_rss_mb", "rs_peak_rss_mb", "Peak RSS", fmtMb);
  }

  // ---- disagreements -------------------------------------------------------
  function renderDisagreements(d) {
    const r = d.real;
    if (!r) return;
    const dc = r.disagreeing_codes;
    const codes = Array.from(new Set(Object.keys(dc.only_hs).concat(Object.keys(dc.only_rs))))
      .sort(function (a, b) { return Number(a) - Number(b); });
    const body = $("codes-diff-body");
    if (!codes.length) {
      body.appendChild(el("tr", null, [el("td", { colspan: "3", class: "muted", text: "No differences." })]));
    }
    codes.forEach(function (c) {
      body.appendChild(el("tr", null, [
        el("td", { class: "mono", text: "SC" + c }),
        el("td", { class: "num", text: fmtNum(dc.only_hs[c] || 0) }),
        el("td", { class: "num", text: fmtNum(dc.only_rs[c] || 0) }),
      ]));
    });
    const wrap = $("mismatch-lists");
    r.repos.forEach(function (x) {
      if (!x.mismatch_files.length) return;
      const ul = el("ul");
      x.mismatch_files.forEach(function (m) {
        const parts = [];
        if (m.only_hs.length) parts.push("only Haskell: " + m.only_hs.join(", "));
        if (m.only_rs.length) parts.push("only Rust: " + m.only_rs.join(", "));
        ul.appendChild(el("li", { text: m.file + "  (" + parts.join("; ") + ")" }));
      });
      wrap.appendChild(el("details", null, [
        el("summary", { text: x.name + ": " + x.mismatch_files.length + " files differ" }),
        ul,
      ]));
    });
  }

  // ---- conformance/machine footers ----------------------------------------
  function renderMeta(d) {
    const m = d.real ? d.real.machine : null;
    const cfg = d.real ? d.real.config : null;
    const rows = [
      ["Reference", d.reference],
      ["Generated", d.generated_at],
    ];
    if (m) {
      rows.push(["CPU", m.cpu + " (" + m.logical_cpus + " threads)"]);
      rows.push(["RAM", m.mem_total_gb + " GB"]);
      rows.push(["Kernel", m.kernel]);
      rows.push(["Rust", m.rs_version]);
      rows.push(["Haskell", Array.isArray(m.hs_version) ? m.hs_version.join(" ") : m.hs_version]);
      rows.push(["Method", m.method + ". " + m.memory_cap + "."]);
      rows.push(["Load average", "start " + (m.loadavg_start || 0).toFixed(2) + ", end " + (m.loadavg_end || 0).toFixed(2) + ". Shared machine, so timings are noisy."]);
    }
    if (cfg) rows.push(["Repos", "pinned commits in bench/repos.toml; " + cfg.runs + " passes per side"]);
    const dl = $("machine-list");
    rows.forEach(function (r) {
      dl.appendChild(el("dt", { text: r[0] }));
      dl.appendChild(el("dd", { text: r[1] || "-" }));
    });
    $("generated-at").textContent = "Data generated " + d.generated_at;
    $("ref-version").textContent = d.reference ? "(" + d.reference.split(" (")[0] + ")" : "(Haskell)";
  }

  // ---- boot ----------------------------------------------------------------
  function boot(d) {
    (d.notes || []).forEach(showNote);
    renderStats(d);
    renderConformance(d);
    renderCorpusCodes(d);
    renderReal(d);
    renderCharts(d);
    renderDisagreements(d);
    renderMeta(d);
  }

  setupTheme();
  fetch("data.json", { cache: "no-cache" })
    .then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    })
    .then(boot)
    .catch(function (err) {
      showNote("Could not load data.json (" + err.message + "). Serve this folder over HTTP, for example: python3 -m http.server -d site");
    });
})();
