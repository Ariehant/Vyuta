// Vyuta parameter tuning view (Phase 4).
//
// Connects to the maestros gateway, requests the parameter list, and renders a
// subsystem-grouped tree. Edits are either applied live (PARAM_SET on change)
// or staged until "Apply". Snapshots can be saved and diffed against the
// current values. Auto-reconnects with backoff (mirrors the telemetry cockpit).
//
// The plan calls for a "React tree view"; this is a dependency-free vanilla DOM
// tree to avoid adding a bundler/React to the fork's plain-tsc extension build
// — the same kind of pragmatic substitution noted elsewhere in the project.
(function () {
  "use strict";

  const cfg = window.__VYUTA_TUNING__ || {};
  const gatewayUrl = cfg.gatewayUrl || "ws://127.0.0.1:9876";

  const $ = (id) => document.getElementById(id);
  const els = {
    status: $("status"),
    paramCount: $("param-count"),
    modifiedCount: $("modified-count"),
    filter: $("filter"),
    liveTune: $("live-tune"),
    apply: $("apply"),
    revert: $("revert"),
    refresh: $("refresh"),
    expand: $("expand"),
    collapse: $("collapse"),
    snapName: $("snap-name"),
    snapSave: $("snap-save"),
    snapSelect: $("snap-select"),
    snapDiff: $("snap-diff"),
    snapDelete: $("snap-delete"),
    diffClear: $("diff-clear"),
    progress: $("progress"),
    progressBar: $("progress-bar"),
    progressLabel: $("progress-label"),
    diffPanel: $("diff-panel"),
    diffName: $("diff-name"),
    diffList: $("diff-list"),
    tree: $("tree"),
    gatewayUrl: $("gateway-url"),
  };
  els.gatewayUrl.textContent = gatewayUrl;
  els.liveTune.checked = !!cfg.liveTune;

  // ---- state --------------------------------------------------------------
  const params = new Map(); // id -> { value, ptype, index }
  const known = new Map(); // id -> last server-confirmed value
  const staged = new Map(); // id -> pending value (when Live Tune off)
  const collapsed = new Set(); // collapsed group names
  let diffEntries = null; // id -> {from,to,kind} when a diff is active
  let total = 0;
  let ws = null;
  let reconnectDelay = 500;
  const MAX_DELAY = 8000;
  let renderTimer = null;

  // ---- helpers ------------------------------------------------------------
  const groupOf = (id) => {
    const i = id.indexOf("_");
    return i > 0 ? id.slice(0, i) : "MISC";
  };
  const ptypeLabel = (t) =>
    ({ 1: "u8", 2: "i8", 3: "u16", 4: "i16", 5: "u32", 6: "i32", 7: "u64", 8: "i64", 9: "f32", 10: "f64" }[t] || "f32");
  const fmt = (v) => {
    if (typeof v !== "number") return "—";
    if (Number.isInteger(v)) return String(v);
    return parseFloat(v.toPrecision(7)).toString();
  };
  const setStatus = (state, label) => {
    els.status.className = "status status--" + state;
    els.status.textContent = label;
  };

  function send(obj) {
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(obj));
  }

  function scheduleRender() {
    if (renderTimer) return;
    renderTimer = setTimeout(() => {
      renderTimer = null;
      renderTree();
    }, 80);
  }

  // ---- counts -------------------------------------------------------------
  function updateCounts() {
    els.paramCount.textContent = params.size + "/" + Math.max(total, params.size);
    els.modifiedCount.textContent = String(staged.size);
    els.apply.disabled = staged.size === 0;
    els.revert.disabled = staged.size === 0;
  }

  // ---- tree render --------------------------------------------------------
  function renderTree() {
    const filter = els.filter.value.trim().toUpperCase();
    const ids = [...params.keys()]
      .filter((id) => !filter || id.includes(filter))
      .sort();

    // Group.
    const groups = new Map();
    for (const id of ids) {
      const g = groupOf(id);
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(id);
    }

    const focusedId = document.activeElement?.dataset?.paramId;
    els.tree.innerHTML = "";
    for (const g of [...groups.keys()].sort()) {
      const groupEl = document.createElement("section");
      groupEl.className = "group";

      const head = document.createElement("button");
      head.className = "group-head";
      const isCollapsed = collapsed.has(g);
      head.innerHTML =
        '<span class="caret">' + (isCollapsed ? "▸" : "▾") + "</span>" +
        '<span class="group-name">' + g + "</span>" +
        '<span class="group-count">' + groups.get(g).length + "</span>";
      head.addEventListener("click", () => {
        if (collapsed.has(g)) collapsed.delete(g);
        else collapsed.add(g);
        renderTree();
      });
      groupEl.appendChild(head);

      if (!isCollapsed) {
        const body = document.createElement("div");
        body.className = "group-body";
        for (const id of groups.get(g)) body.appendChild(renderRow(id));
        groupEl.appendChild(body);
      }
      els.tree.appendChild(groupEl);
    }

    if (focusedId) {
      const again = els.tree.querySelector('[data-param-id="' + cssEscape(focusedId) + '"]');
      if (again) again.focus();
    }
  }

  function renderRow(id) {
    const p = params.get(id);
    const row = document.createElement("div");
    row.className = "row";
    const dirty = staged.has(id);
    if (dirty) row.classList.add("row--dirty");
    const d = diffEntries && diffEntries.get(id);
    if (d) row.classList.add("row--diff-" + d.kind);

    const name = document.createElement("span");
    name.className = "row-name";
    name.textContent = id;
    name.title = id + "  (" + ptypeLabel(p.ptype) + ")";

    const input = document.createElement("input");
    input.type = "number";
    input.step = "any";
    input.className = "row-input";
    input.dataset.paramId = id;
    input.value = dirty ? staged.get(id) : fmt(p.value);
    input.addEventListener("change", () => onEdit(id, input));
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") onEdit(id, input);
    });

    const meta = document.createElement("span");
    meta.className = "row-meta";
    if (d) {
      meta.textContent = (d.from == null ? "∅" : fmt(d.from)) + " → " + (d.to == null ? "∅" : fmt(d.to));
    } else {
      meta.textContent = ptypeLabel(p.ptype);
    }

    row.append(name, input, meta);
    return row;
  }

  function onEdit(id, input) {
    const v = Number(input.value);
    if (!Number.isFinite(v)) return;
    if (els.liveTune.checked) {
      staged.delete(id);
      send({ cmd: "set_param", id, value: v });
    } else {
      const cur = known.has(id) ? known.get(id) : params.get(id).value;
      if (Math.abs(v - cur) < 1e-9) staged.delete(id);
      else staged.set(id, v);
    }
    updateCounts();
    // Update just this row's dirty class without a full re-render (keep focus).
    const row = input.closest(".row");
    if (row) row.classList.toggle("row--dirty", staged.has(id));
  }

  // ---- message handlers ---------------------------------------------------
  function onParamValue(m) {
    params.set(m.id, { value: m.value, ptype: m.param_type, index: m.index });
    known.set(m.id, m.value);
    if (typeof m.count === "number") total = Math.max(total, m.count);
    // If the user has a pending edit equal to the confirmed value, clear it.
    if (staged.has(m.id) && Math.abs(staged.get(m.id) - m.value) < 1e-9) staged.delete(m.id);
    scheduleRender();
    updateCounts();
  }

  function onProgress(m) {
    total = Math.max(total, m.total || 0);
    const have = m.received || params.size;
    if (have < total && total > 0) {
      els.progress.classList.remove("hidden");
      els.progressBar.style.width = Math.round((have / total) * 100) + "%";
      els.progressLabel.textContent = have + " / " + total;
    } else {
      els.progress.classList.add("hidden");
    }
    updateCounts();
  }

  function onSnapshotList(m) {
    const cur = els.snapSelect.value;
    els.snapSelect.innerHTML = '<option value="">— snapshot —</option>';
    for (const n of m.names || []) {
      const o = document.createElement("option");
      o.value = n;
      o.textContent = n;
      els.snapSelect.appendChild(o);
    }
    if ((m.names || []).includes(cur)) els.snapSelect.value = cur;
  }

  function onSnapshotDiff(m) {
    diffEntries = new Map();
    for (const e of m.entries || []) diffEntries.set(e.id, e);
    els.diffName.textContent = m.name;
    els.diffList.innerHTML = "";
    if ((m.entries || []).length === 0) {
      els.diffList.innerHTML = '<div class="diff-empty">no differences</div>';
    }
    for (const e of m.entries || []) {
      const div = document.createElement("div");
      div.className = "diff-row diff-row--" + e.kind;
      div.innerHTML =
        '<span class="diff-id">' + e.id + "</span>" +
        '<span class="diff-kind">' + e.kind + "</span>" +
        '<span class="diff-vals">' + (e.from == null ? "∅" : fmt(e.from)) + " → " + (e.to == null ? "∅" : fmt(e.to)) + "</span>";
      els.diffList.appendChild(div);
    }
    els.diffPanel.classList.remove("hidden");
    els.diffClear.classList.remove("hidden");
    renderTree();
  }

  function clearDiff() {
    diffEntries = null;
    els.diffPanel.classList.add("hidden");
    els.diffClear.classList.add("hidden");
    renderTree();
  }

  // ---- websocket ----------------------------------------------------------
  function connect() {
    setStatus("connecting", "connecting…");
    try {
      ws = new WebSocket(gatewayUrl);
    } catch (_e) {
      scheduleReconnect();
      return;
    }
    ws.addEventListener("open", () => {
      setStatus("connected", "connected");
      reconnectDelay = 500;
      send({ cmd: "request_params" });
      send({ cmd: "list_snapshots" });
    });
    ws.addEventListener("message", (ev) => {
      let m;
      try {
        m = JSON.parse(ev.data);
      } catch (_e) {
        return;
      }
      if (!m.type) return; // telemetry frame — ignore
      switch (m.type) {
        case "param_value": onParamValue(m); break;
        case "param_progress": onProgress(m); break;
        case "snapshot_list": onSnapshotList(m); break;
        case "snapshot_diff": onSnapshotDiff(m); break;
        case "param_ack": /* could surface errors; ignored for now */ break;
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

  // ---- controls -----------------------------------------------------------
  els.filter.addEventListener("input", scheduleRender);
  els.refresh.addEventListener("click", () => send({ cmd: "request_params" }));
  els.expand.addEventListener("click", () => {
    collapsed.clear();
    renderTree();
  });
  els.collapse.addEventListener("click", () => {
    for (const id of params.keys()) collapsed.add(groupOf(id));
    renderTree();
  });
  els.apply.addEventListener("click", () => {
    for (const [id, v] of staged) send({ cmd: "set_param", id, value: v });
    staged.clear();
    updateCounts();
    renderTree();
  });
  els.revert.addEventListener("click", () => {
    staged.clear();
    updateCounts();
    renderTree();
  });
  els.snapSave.addEventListener("click", () => {
    const name = (els.snapName.value || "").trim() || ("snap-" + new Date().toISOString().slice(11, 19));
    send({ cmd: "save_snapshot", name });
    els.snapName.value = "";
  });
  els.snapDiff.addEventListener("click", () => {
    const name = els.snapSelect.value;
    if (name) send({ cmd: "diff_snapshot", name });
  });
  els.snapDelete.addEventListener("click", () => {
    const name = els.snapSelect.value;
    if (name) {
      send({ cmd: "delete_snapshot", name });
      if (diffEntries) clearDiff();
    }
  });
  els.diffClear.addEventListener("click", clearDiff);
  els.liveTune.addEventListener("change", () => {
    // Switching to live applies nothing automatically; staged edits remain.
    updateCounts();
  });

  function cssEscape(s) {
    return (window.CSS && CSS.escape) ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&");
  }

  updateCounts();
  connect();
})();
