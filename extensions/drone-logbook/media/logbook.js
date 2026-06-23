// Vyuta flight-log analyzer (Phase 5).
//
// Connects to the logbook sidecar, shows the auto-review checklist, a flight-
// mode timeline, a field picker, and stacked uPlot charts (with mode bands and
// synced cursors). Auto-reconnects with backoff.
(function () {
  "use strict";

  const cfg = window.__VYUTA_LOG__ || {};
  const serverUrl = cfg.serverUrl || "ws://127.0.0.1:9878";
  const uPlot = window.uPlot;

  const $ = (id) => document.getElementById(id);
  const els = {
    status: $("status"),
    logName: $("log-name"),
    logDur: $("log-dur"),
    source: $("source"),
    path: $("path"),
    load: $("load"),
    synthetic: $("synthetic"),
    review: $("review"),
    timeline: $("timeline"),
    legend: $("timeline-legend"),
    fieldFilter: $("field-filter"),
    fields: $("fields"),
    charts: $("charts"),
    chartsHint: $("charts-hint"),
    messages: $("messages"),
    serverUrl: $("server-url"),
  };
  els.serverUrl.textContent = serverUrl;

  let ws = null;
  let reconnectDelay = 500;
  const MAX_DELAY = 8000;

  let overview = null;
  let modes = [];
  let duration = 0;
  const selected = new Set();
  const charts = new Map(); // name -> uPlot
  const data = new Map(); // name -> {t, v}
  let reqTimer = null;
  const SYNC = uPlot ? uPlot.sync("vyuta-log") : null;

  // ---- helpers ------------------------------------------------------------
  const setStatus = (s, label) => {
    els.status.className = "status status--" + s;
    els.status.textContent = label;
  };
  const send = (o) => {
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(o));
  };
  const groupOf = (name) => {
    const i = name.indexOf("[");
    return i > 0 ? name.slice(0, i) : name;
  };
  // Deterministic color per mode/string.
  function hue(str) {
    let h = 0;
    for (let i = 0; i < str.length; i++) h = (h * 31 + str.charCodeAt(i)) % 360;
    return h;
  }
  const modeColor = (m, a) => `hsla(${hue(m)},60%,55%,${a})`;

  // ---- review -------------------------------------------------------------
  function renderReview(findings) {
    els.review.innerHTML = "";
    for (const f of findings) {
      const row = document.createElement("div");
      row.className = "finding finding--" + f.severity;
      row.innerHTML =
        '<span class="sev sev--' + f.severity + '">' + f.severity + "</span>" +
        '<span class="finding-title">' + f.title + "</span>" +
        '<span class="finding-detail">' + f.detail + "</span>";
      els.review.appendChild(row);
    }
  }

  // ---- timeline -----------------------------------------------------------
  function renderTimeline() {
    els.timeline.innerHTML = "";
    els.legend.innerHTML = "";
    if (!duration || modes.length === 0) {
      els.timeline.innerHTML = '<span class="muted">no mode data</span>';
      return;
    }
    const seen = new Set();
    for (const m of modes) {
      const span = document.createElement("div");
      span.className = "mode-span";
      span.style.left = (m.t0 / duration) * 100 + "%";
      span.style.width = Math.max(((m.t1 - m.t0) / duration) * 100, 0.5) + "%";
      span.style.background = modeColor(m.mode, 0.85);
      span.title = `${m.mode}  ${m.t0.toFixed(1)}–${m.t1.toFixed(1)}s`;
      els.timeline.appendChild(span);
      seen.add(m.mode);
    }
    for (const mode of seen) {
      const chip = document.createElement("span");
      chip.className = "legend-chip";
      chip.innerHTML = '<i style="background:' + modeColor(mode, 0.85) + '"></i>' + mode;
      els.legend.appendChild(chip);
    }
  }

  // ---- field picker -------------------------------------------------------
  function renderFields() {
    const filter = els.fieldFilter.value.trim().toLowerCase();
    const summaries = (overview?.series || []).filter(
      (s) => !filter || s.name.toLowerCase().includes(filter)
    );
    const groups = new Map();
    for (const s of summaries) {
      const g = groupOf(s.name);
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(s);
    }
    els.fields.innerHTML = "";
    for (const g of [...groups.keys()].sort()) {
      const det = document.createElement("details");
      det.open = !!filter;
      const sum = document.createElement("summary");
      sum.textContent = g + " (" + groups.get(g).length + ")";
      det.appendChild(sum);
      for (const s of groups.get(g)) {
        const label = document.createElement("label");
        label.className = "field-item";
        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.checked = selected.has(s.name);
        cb.addEventListener("change", () => toggleField(s.name, cb.checked));
        const span = document.createElement("span");
        span.textContent = s.name.slice(g.length);
        span.title = s.name + `  (${s.count} pts, ${fmt(s.min)}…${fmt(s.max)})`;
        label.append(cb, span);
        det.appendChild(label);
      }
      els.fields.appendChild(det);
    }
  }

  const fmt = (v) => (typeof v === "number" ? parseFloat(v.toPrecision(5)).toString() : "—");

  function toggleField(name, on) {
    if (on) selected.add(name);
    else {
      selected.delete(name);
      const c = charts.get(name);
      if (c) {
        c.destroy();
        charts.delete(name);
      }
      const host = document.getElementById("chart-" + cssId(name));
      if (host) host.remove();
      data.delete(name);
    }
    els.chartsHint.classList.toggle("hidden", selected.size > 0);
    requestSeries();
  }

  function requestSeries() {
    if (reqTimer) clearTimeout(reqTimer);
    reqTimer = setTimeout(() => {
      const names = [...selected].filter((n) => !data.has(n));
      if (names.length) send({ cmd: "series", names, max_points: 4000 });
    }, 60);
  }

  // ---- charts -------------------------------------------------------------
  function modeBandsPlugin() {
    return {
      hooks: {
        drawClear: (u) => {
          if (!modes.length) return;
          const ctx = u.ctx;
          ctx.save();
          for (const m of modes) {
            const x0 = u.valToPos(m.t0, "x", true);
            const x1 = u.valToPos(m.t1, "x", true);
            ctx.fillStyle = modeColor(m.mode, 0.1);
            ctx.fillRect(x0, u.bbox.top, Math.max(x1 - x0, 1), u.bbox.height);
          }
          ctx.restore();
        },
      },
    };
  }

  function buildChart(name) {
    const d = data.get(name);
    if (!d) return;
    const host = document.createElement("div");
    host.className = "chart";
    host.id = "chart-" + cssId(name);
    const title = document.createElement("div");
    title.className = "chart-title";
    title.textContent = name;
    const plot = document.createElement("div");
    host.append(title, plot);
    els.charts.appendChild(host);

    const stroke = getComputedStyle(document.body).getPropertyValue("--vscode-charts-blue") || "#4f9dff";
    const fg = getComputedStyle(document.body).getPropertyValue("--vscode-foreground") || "#ccc";
    const grid = "rgba(128,128,128,0.15)";
    const width = els.charts.clientWidth || 600;

    const opts = {
      width,
      height: 150,
      scales: { x: { time: false } },
      cursor: { sync: SYNC ? { key: SYNC.key } : undefined },
      legend: { show: true },
      plugins: [modeBandsPlugin()],
      axes: [
        { stroke: fg.trim(), grid: { stroke: grid }, ticks: { stroke: grid } },
        { stroke: fg.trim(), grid: { stroke: grid }, ticks: { stroke: grid }, size: 60 },
      ],
      series: [
        { label: "t (s)" },
        { label: name.split(".").pop(), stroke: stroke.trim() || "#4f9dff", width: 1.25, points: { show: false } },
      ],
    };
    const u = new uPlot(opts, [d.t, d.v], plot);
    charts.set(name, u);
  }

  function onSeries(map) {
    for (const [name, sd] of Object.entries(map)) {
      data.set(name, sd);
      if (!charts.has(name) && selected.has(name)) buildChart(name);
    }
  }

  function resizeCharts() {
    const w = els.charts.clientWidth || 600;
    for (const u of charts.values()) u.setSize({ width: w, height: 150 });
  }
  window.addEventListener("resize", resizeCharts);

  // ---- messages -----------------------------------------------------------
  function renderMessages(msgs) {
    els.messages.innerHTML = "";
    if (!msgs || msgs.length === 0) {
      els.messages.innerHTML = '<span class="muted">no logged messages</span>';
      return;
    }
    for (const m of msgs) {
      const row = document.createElement("div");
      row.className = "msg msg--lvl" + m.level;
      row.innerHTML =
        '<span class="msg-t">' + m.t.toFixed(1) + "s</span>" +
        '<span class="msg-text">' + escapeHtml(m.text) + "</span>";
      els.messages.appendChild(row);
    }
  }

  // ---- overview -----------------------------------------------------------
  function onOverview(m) {
    overview = m;
    modes = m.modes || [];
    duration = m.duration_s || 0;
    els.logName.textContent = m.name || "—";
    els.logDur.textContent = duration.toFixed(1) + "s";
    els.source.textContent = m.source || "";
    // Drop charts/data for series no longer present (e.g. after Load).
    for (const name of [...charts.keys()]) {
      const c = charts.get(name);
      if (c) c.destroy();
    }
    charts.clear();
    data.clear();
    els.charts.innerHTML = "";
    renderTimeline();
    renderFields();
    renderMessages(m.messages);
    els.chartsHint.classList.toggle("hidden", selected.size > 0);
    // Re-request any still-selected series for the new log.
    requestSeries();
  }

  // ---- websocket ----------------------------------------------------------
  function connect() {
    setStatus("connecting", "connecting…");
    try {
      ws = new WebSocket(serverUrl);
    } catch (_e) {
      scheduleReconnect();
      return;
    }
    ws.addEventListener("open", () => {
      setStatus("connected", "connected");
      reconnectDelay = 500;
    });
    ws.addEventListener("message", (ev) => {
      let m;
      try {
        m = JSON.parse(ev.data);
      } catch (_e) {
        return;
      }
      switch (m.type) {
        case "overview": onOverview(m); break;
        case "review": renderReview(m.findings || []); break;
        case "series": onSeries(m.series || {}); break;
        case "error": setStatus("connected", "error: " + m.message); break;
        default: break;
      }
    });
    ws.addEventListener("close", () => {
      setStatus("disconnected", "disconnected");
      scheduleReconnect();
    });
    ws.addEventListener("error", () => {
      try {
        ws.close();
      } catch (_e) {
        /* no-op */
      }
    });
  }
  function scheduleReconnect() {
    setTimeout(connect, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, MAX_DELAY);
  }

  // ---- misc helpers -------------------------------------------------------
  function cssId(s) {
    return s.replace(/[^a-zA-Z0-9_-]/g, "_");
  }
  function escapeHtml(s) {
    return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  }

  // ---- controls -----------------------------------------------------------
  els.load.addEventListener("click", () => {
    const p = els.path.value.trim();
    if (p) send({ cmd: "load", path: p });
  });
  els.path.addEventListener("keydown", (e) => {
    if (e.key === "Enter") els.load.click();
  });
  els.synthetic.addEventListener("click", () => send({ cmd: "synthetic" }));
  els.fieldFilter.addEventListener("input", renderFields);

  connect();
})();
