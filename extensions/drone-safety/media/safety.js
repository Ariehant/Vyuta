// Vyuta pre-flight & safety view (Phase 7).
//
// Polls the maestros pre-flight checklist, gates the Arm button on it, and
// arms/disarms. Tracks armed state from telemetry frames; sounds an alarm on
// arm and when a check fails while armed. Auto-reconnects.
(function () {
  "use strict";

  const cfg = window.__VYUTA_SAFETY__ || {};
  const gatewayUrl = cfg.gatewayUrl || "ws://127.0.0.1:9876";
  const audible = cfg.audibleAlarms !== false;

  const $ = (id) => document.getElementById(id);
  const els = {
    status: $("status"),
    armed: $("armed"),
    banner: $("banner"),
    checklist: $("checklist"),
    refresh: $("refresh"),
    arm: $("arm"),
    disarm: $("disarm"),
    armMsg: $("arm-msg"),
    gatewayUrl: $("gateway-url"),
  };
  els.gatewayUrl.textContent = gatewayUrl;

  let ws = null;
  let reconnectDelay = 500;
  const MAX_DELAY = 8000;
  let preflightOk = false;
  let isArmed = false;
  let pollTimer = null;

  const setStatus = (s, label) => {
    els.status.className = "status status--" + s;
    els.status.textContent = label;
  };
  const send = (o) => {
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(o));
  };

  // ---- audible alarm ------------------------------------------------------
  let audioCtx = null;
  let lastBeep = 0;
  function beep(freq) {
    if (!audible) return;
    const now = performance.now();
    if (now - lastBeep < 600) return;
    lastBeep = now;
    try {
      if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      if (audioCtx.state === "suspended") audioCtx.resume();
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.frequency.value = freq || 880;
      gain.gain.value = 0.05;
      osc.connect(gain).connect(audioCtx.destination);
      osc.start();
      osc.stop(audioCtx.currentTime + 0.18);
    } catch (_e) {
      /* visual alarm still applies */
    }
  }

  function renderChecklist(items) {
    els.checklist.innerHTML = "";
    for (const it of items) {
      const row = document.createElement("div");
      row.className = "check " + (it.pass ? "check--pass" : "check--fail");
      row.innerHTML =
        '<span class="check-icon">' + (it.pass ? "✓" : "✗") + "</span>" +
        '<span class="check-label">' + it.label + "</span>" +
        '<span class="check-detail">' + it.detail + "</span>";
      els.checklist.appendChild(row);
    }
  }

  function updateButtons() {
    els.arm.disabled = !(preflightOk && !isArmed);
    els.disarm.disabled = !isArmed;
    els.armed.classList.toggle("hidden", !isArmed);
    els.banner.className = "banner " + (isArmed ? "banner--armed" : "hidden");
    if (isArmed) els.banner.textContent = "⚠ ARMED — propellers may spin";
  }

  function onPreflight(m) {
    preflightOk = !!m.ok;
    renderChecklist(m.items || []);
    updateButtons();
    if (isArmed && !preflightOk) beep(440); // safety regression while armed
  }

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
      send({ cmd: "request_params" }); // ensure params sync for the checklist
      send({ cmd: "preflight" });
      clearInterval(pollTimer);
      pollTimer = setInterval(() => send({ cmd: "preflight" }), 1000);
    });
    ws.addEventListener("message", (ev) => {
      let m;
      try {
        m = JSON.parse(ev.data);
      } catch (_e) {
        return;
      }
      if (!m.type) {
        // telemetry frame
        if (typeof m.armed === "boolean") {
          isArmed = m.armed;
          updateButtons();
        }
        return;
      }
      if (m.type === "preflight") onPreflight(m);
      else if (m.type === "arm_ack") {
        els.armMsg.textContent = (m.ok ? "✓ " : "✗ ") + m.message;
        els.armMsg.className = "arm-msg " + (m.ok ? "ok" : "err");
        if (m.ok && m.armed) beep(880);
      }
    });
    ws.addEventListener("close", () => {
      setStatus("disconnected", "disconnected");
      clearInterval(pollTimer);
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

  els.arm.addEventListener("click", () => send({ cmd: "arm" }));
  els.disarm.addEventListener("click", () => send({ cmd: "disarm" }));
  els.refresh.addEventListener("click", () => send({ cmd: "preflight" }));

  updateButtons();
  connect();
})();
