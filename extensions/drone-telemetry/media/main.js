// Vyuta telemetry webview client (Phase 0).
//
// Connects to the maestros gateway WebSocket, parses JSON telemetry frames,
// and updates the live readout. Auto-reconnects with backoff if the socket
// drops. Phase 1 replaces the JSON parse + text readout with FlatBuffers and a
// Three.js attitude indicator.
(function () {
  "use strict";

  const gatewayUrl = window.__VYUTA_GATEWAY_URL__ || "ws://127.0.0.1:9876";

  const els = {
    status: document.getElementById("status"),
    syntheticBadge: document.getElementById("synthetic-badge"),
    gatewayUrl: document.getElementById("gateway-url"),
    frameCount: document.getElementById("frame-count"),
    roll: document.getElementById("roll"),
    pitch: document.getElementById("pitch"),
    yaw: document.getElementById("yaw"),
    lat: document.getElementById("lat"),
    lon: document.getElementById("lon"),
    alt: document.getElementById("alt"),
    battery_v: document.getElementById("battery_v"),
    battery_pct: document.getElementById("battery_pct"),
    mode: document.getElementById("mode"),
    armed: document.getElementById("armed"),
  };

  els.gatewayUrl.textContent = gatewayUrl;

  let frameCount = 0;
  let reconnectDelay = 500; // ms, doubles up to a cap
  const MAX_DELAY = 8000;

  const deg = (rad) => ((rad * 180) / Math.PI).toFixed(1) + "°";

  function setStatus(state, label) {
    els.status.className = "status status--" + state;
    els.status.textContent = label;
  }

  function render(f) {
    frameCount++;
    els.frameCount.textContent = String(frameCount);
    els.roll.textContent = deg(f.roll);
    els.pitch.textContent = deg(f.pitch);
    els.yaw.textContent = deg(f.yaw);
    els.lat.textContent = f.lat.toFixed(6);
    els.lon.textContent = f.lon.toFixed(6);
    els.alt.textContent = f.alt_m.toFixed(1) + " m";
    els.battery_v.textContent = f.battery_v.toFixed(2) + " V";
    els.battery_pct.textContent = Math.round(f.battery_pct) + " %";
    els.mode.textContent = f.mode;
    els.armed.textContent = f.armed ? "ARMED" : "disarmed";
    els.armed.classList.toggle("armed", !!f.armed);
    els.syntheticBadge.classList.toggle("hidden", !f.synthetic);
  }

  function connect() {
    setStatus("connecting", "connecting…");
    let ws;
    try {
      ws = new WebSocket(gatewayUrl);
    } catch (err) {
      scheduleReconnect();
      return;
    }

    ws.addEventListener("open", () => {
      setStatus("connected", "connected");
      reconnectDelay = 500;
    });

    ws.addEventListener("message", (ev) => {
      try {
        render(JSON.parse(ev.data));
      } catch (_err) {
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
      } catch (_err) {
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
