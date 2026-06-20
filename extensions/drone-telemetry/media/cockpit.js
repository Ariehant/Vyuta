// Vyuta telemetry cockpit orchestrator (Phase 1).
//
// Connects to the maestros gateway, drives the artificial horizon + GPS map,
// updates the readouts/battery gauge, and runs the alarm system (low battery /
// link loss, visual + optional audible). Auto-reconnects with backoff.
(function () {
  "use strict";

  const cfg = window.__VYUTA__ || {};
  const gatewayUrl = cfg.gatewayUrl || "ws://127.0.0.1:9876";
  const warnPct = typeof cfg.batteryWarnPercent === "number" ? cfg.batteryWarnPercent : 25;
  const critPct = typeof cfg.batteryCriticalPercent === "number" ? cfg.batteryCriticalPercent : 15;
  const audible = cfg.audibleAlarms !== false;

  const $ = (id) => document.getElementById(id);
  const els = {
    status: $("status"),
    sourceBadge: $("source-badge"),
    mode: $("mode"),
    armed: $("armed"),
    alarm: $("alarm"),
    gatewayUrl: $("gateway-url"),
    frameCount: $("frame-count"),
    frameRate: $("frame-rate"),
    roll: $("roll"),
    pitch: $("pitch"),
    heading: $("heading"),
    lat: $("lat"),
    lon: $("lon"),
    alt: $("alt"),
    batteryFill: $("battery-fill"),
    batteryPctLabel: $("battery-pct-label"),
    battery_v: $("battery_v"),
    current: $("current"),
    groundspeed: $("groundspeed"),
    airspeed: $("airspeed"),
    climb: $("climb"),
    throttle: $("throttle"),
    system_status: $("system_status"),
    link: $("link"),
  };
  els.gatewayUrl.textContent = gatewayUrl;

  const horizon = new window.VyutaAttitude($("horizon"));
  const map = new window.VyutaMap("map");
  setTimeout(() => map.invalidate(), 200);
  window.addEventListener("resize", () => map.invalidate());

  let latest = null;
  let frameCount = 0;
  let rateWindow = [];
  let lastMapUpdate = 0;
  let reconnectDelay = 500;
  const MAX_DELAY = 8000;

  // ---- formatting helpers -------------------------------------------------
  const degOf = (rad) => ((rad * 180) / Math.PI).toFixed(1) + "°";
  const num = (v, digits, unit) =>
    typeof v === "number" ? v.toFixed(digits) + (unit || "") : "—";

  // ---- audible alarm ------------------------------------------------------
  let audioCtx = null;
  let lastBeep = 0;
  function beep() {
    if (!audible) return;
    const now = performance.now();
    if (now - lastBeep < 1000) return;
    lastBeep = now;
    try {
      if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      if (audioCtx.state === "suspended") audioCtx.resume();
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.frequency.value = 880;
      gain.gain.value = 0.05;
      osc.connect(gain).connect(audioCtx.destination);
      osc.start();
      osc.stop(audioCtx.currentTime + 0.18);
    } catch (_e) {
      /* audio unavailable; visual alarm still applies */
    }
  }

  // ---- alarm evaluation ---------------------------------------------------
  function evaluateAlarms(f) {
    let level = "none";
    let msg = "";
    if (!f.link_ok) {
      level = "critical";
      msg = "LINK LOST — no MAVLink heartbeat";
    } else if (typeof f.battery_pct === "number" && f.battery_pct <= critPct) {
      level = "critical";
      msg = "BATTERY CRITICAL — " + Math.round(f.battery_pct) + "%";
    } else if (typeof f.battery_pct === "number" && f.battery_pct <= warnPct) {
      level = "warn";
      msg = "LOW BATTERY — " + Math.round(f.battery_pct) + "%";
    }

    if (level === "none") {
      els.alarm.className = "alarm hidden";
      els.alarm.textContent = "";
    } else {
      els.alarm.className = "alarm alarm--" + level;
      els.alarm.textContent = msg;
      if (level === "critical") beep();
    }
  }

  // ---- DOM render ---------------------------------------------------------
  function renderReadouts(f) {
    els.roll.textContent = degOf(f.roll);
    els.pitch.textContent = degOf(f.pitch);
    els.heading.textContent = num(f.heading_deg, 0, "°");
    els.lat.textContent = typeof f.lat === "number" ? f.lat.toFixed(6) : "—";
    els.lon.textContent = typeof f.lon === "number" ? f.lon.toFixed(6) : "—";
    const alt = typeof f.rel_alt_m === "number" ? f.rel_alt_m : f.alt_m;
    els.alt.textContent = num(alt, 1, " m");

    els.battery_v.textContent = num(f.battery_v, 2, " V");
    els.current.textContent = num(f.current_a, 1, " A");
    if (typeof f.battery_pct === "number") {
      const pct = Math.max(0, Math.min(100, f.battery_pct));
      els.batteryFill.style.width = pct + "%";
      els.batteryPctLabel.textContent = Math.round(pct) + "%";
      els.batteryFill.className =
        "gauge-fill " +
        (pct <= critPct ? "gauge-fill--crit" : pct <= warnPct ? "gauge-fill--warn" : "gauge-fill--ok");
    } else {
      els.batteryPctLabel.textContent = "—";
    }

    els.groundspeed.textContent = num(f.groundspeed_mps, 1, " m/s");
    els.airspeed.textContent = num(f.airspeed_mps, 1, " m/s");
    els.climb.textContent = num(f.climb_mps, 1, " m/s");
    els.throttle.textContent = num(f.throttle_pct, 0, " %");
    els.system_status.textContent = f.system_status || "—";
    els.link.textContent = f.link_ok
      ? "OK" + (typeof f.heartbeat_age_ms === "number" ? " (" + f.heartbeat_age_ms + " ms)" : "")
      : "LOST";

    els.mode.textContent = f.mode || "—";
    els.armed.classList.toggle("hidden", !f.armed);
    if (f.source) {
      els.sourceBadge.textContent = f.source.toUpperCase();
      els.sourceBadge.className = "badge badge--" + f.source;
    }
  }

  // ---- render loop --------------------------------------------------------
  function loop() {
    if (latest) {
      const f = latest;
      horizon.update(f.roll, f.pitch, f.heading_deg);
      horizon.render();
      renderReadouts(f);
      evaluateAlarms(f);

      const now = performance.now();
      if (now - lastMapUpdate > 200) {
        lastMapUpdate = now;
        map.update(f.lat, f.lon, f.heading_deg);
      }
    }
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);

  // ---- frame-rate display -------------------------------------------------
  setInterval(() => {
    const now = performance.now();
    rateWindow = rateWindow.filter((t) => now - t < 1000);
    els.frameRate.textContent = String(rateWindow.length);
  }, 500);

  // ---- websocket ----------------------------------------------------------
  function setStatus(state, label) {
    els.status.className = "status status--" + state;
    els.status.textContent = label;
  }

  function connect() {
    setStatus("connecting", "connecting…");
    let ws;
    try {
      ws = new WebSocket(gatewayUrl);
    } catch (_e) {
      scheduleReconnect();
      return;
    }
    ws.addEventListener("open", () => {
      setStatus("connected", "connected");
      reconnectDelay = 500;
    });
    ws.addEventListener("message", (ev) => {
      try {
        latest = JSON.parse(ev.data);
        frameCount++;
        rateWindow.push(performance.now());
        els.frameCount.textContent = String(frameCount);
      } catch (_e) {
        /* ignore malformed frame */
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

  connect();
})();
