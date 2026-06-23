// Vyuta simulation control orchestrator (Phase 3).
//
// Connects to the sim-manager sidecar, populates the world/vehicle pickers from
// its catalogue, drives start/stop/reset + wind + the mission REPL, streams the
// log console and status, and feeds pose frames to the 3D viewport. Auto-
// reconnects with backoff (mirrors the telemetry cockpit).

import { Viewport3D } from "./viewport3d.js";

const cfg = window.__VYUTA_SIM__ || {};
const managerUrl = cfg.managerUrl || "ws://127.0.0.1:9877";

const $ = (id) => document.getElementById(id);
const els = {
  status: $("status"),
  mockBadge: $("mock-badge"),
  phase: $("phase"),
  flightMode: $("flight-mode"),
  armed: $("armed"),
  world: $("world"),
  vehicle: $("vehicle"),
  headless: $("headless"),
  forceMock: $("force-mock"),
  btnStart: $("btn-start"),
  btnStop: $("btn-stop"),
  btnReset: $("btn-reset"),
  makeTarget: $("make-target"),
  pid: $("pid"),
  simTime: $("sim-time"),
  message: $("message"),
  windSpeed: $("wind-speed"),
  windDir: $("wind-dir"),
  windGust: $("wind-gust"),
  windSpeedVal: $("wind-speed-val"),
  windDirVal: $("wind-dir-val"),
  windGustVal: $("wind-gust-val"),
  replInput: $("repl-input"),
  replSend: $("repl-send"),
  log: $("log"),
  logClear: $("log-clear"),
  managerUrl: $("manager-url"),
  poseRate: $("pose-rate"),
  ovX: $("ov-x"),
  ovY: $("ov-y"),
  ovZ: $("ov-z"),
  ovSpd: $("ov-spd"),
};
els.managerUrl.textContent = managerUrl;
els.headless.checked = cfg.headless !== false;
els.forceMock.checked = !!cfg.forceMock;

const viewport = new Viewport3D($("viewport"));

let ws = null;
let reconnectDelay = 500;
const MAX_DELAY = 8000;
let catalogReady = false;
let poseTimes = [];

// ---- helpers --------------------------------------------------------------
const numFmt = (v, d, u) => (typeof v === "number" ? v.toFixed(d) + (u || "") : "—");

function setStatus(state, label) {
  els.status.className = "status status--" + state;
  els.status.textContent = label;
}

function send(obj) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(obj));
  }
}

function appendLog(stream, line) {
  const div = document.createElement("div");
  div.className = "log-line log-line--" + (stream || "sim");
  div.textContent = line;
  els.log.appendChild(div);
  // Cap the DOM log.
  while (els.log.childElementCount > 500) els.log.removeChild(els.log.firstChild);
  els.log.scrollTop = els.log.scrollHeight;
}

function fillSelect(sel, entries, preferred) {
  sel.innerHTML = "";
  for (const e of entries) {
    const opt = document.createElement("option");
    opt.value = e.id;
    opt.textContent = e.label;
    opt.title = e.description || "";
    sel.appendChild(opt);
  }
  if (preferred && entries.some((e) => e.id === preferred)) sel.value = preferred;
}

// ---- frame handlers -------------------------------------------------------
function onCatalog(m) {
  fillSelect(els.world, m.worlds || [], cfg.defaultWorld);
  fillSelect(els.vehicle, m.vehicles || [], cfg.defaultVehicle);
  catalogReady = true;
}

function onStatus(m) {
  els.phase.textContent = m.phase;
  els.phase.className = "pill pill--" + m.phase;
  els.flightMode.textContent = m.flight_mode || "—";
  els.armed.classList.toggle("hidden", !m.armed);
  els.mockBadge.classList.toggle("hidden", !m.mock);
  els.makeTarget.textContent = m.make_target || "—";
  els.pid.textContent = m.pid == null ? "—" : String(m.pid);
  els.simTime.textContent = numFmt(m.sim_time_s, 1, " s");
  els.message.textContent = m.message || "—";

  const running = m.phase === "running" || m.phase === "starting";
  els.btnStart.disabled = running;
  els.btnStop.disabled = !running;

  // Reflect wind back into the sliders only if the user isn't dragging.
  if (m.wind && document.activeElement !== els.windSpeed &&
      document.activeElement !== els.windDir && document.activeElement !== els.windGust) {
    els.windSpeed.value = m.wind.speed_mps;
    els.windDir.value = m.wind.direction_deg;
    els.windGust.value = Math.round((m.wind.gust || 0) * 100);
    refreshWindLabels();
  }
}

function onPose(m) {
  viewport.update(m);
  els.ovX.textContent = numFmt(m.x, 1);
  els.ovY.textContent = numFmt(m.y, 1);
  els.ovZ.textContent = numFmt(m.z, 1, " m");
  const spd = Math.hypot(m.vx || 0, m.vy || 0, m.vz || 0);
  els.ovSpd.textContent = numFmt(spd, 1, " m/s");
  poseTimes.push(performance.now());
}

// ---- websocket ------------------------------------------------------------
function connect() {
  setStatus("connecting", "connecting…");
  try {
    ws = new WebSocket(managerUrl);
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
      case "catalog": onCatalog(m); break;
      case "status": onStatus(m); break;
      case "pose": onPose(m); break;
      case "log": appendLog(m.stream, m.line); break;
      case "ack":
        appendLog("sim", (m.ok ? "✓ " : "✗ ") + m.cmd + ": " + m.message);
        break;
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

// ---- controls -------------------------------------------------------------
els.btnStart.addEventListener("click", () => {
  send({
    cmd: "start",
    world: catalogReady ? els.world.value : cfg.defaultWorld,
    vehicle: catalogReady ? els.vehicle.value : cfg.defaultVehicle,
    headless: els.headless.checked,
    mock: els.forceMock.checked ? true : null,
  });
});
els.btnStop.addEventListener("click", () => send({ cmd: "stop" }));
els.btnReset.addEventListener("click", () => {
  send({ cmd: "reset" });
  viewport.reset();
});

function refreshWindLabels() {
  els.windSpeedVal.textContent = Number(els.windSpeed.value).toFixed(1) + " m/s";
  els.windDirVal.textContent = els.windDir.value + "°";
  els.windGustVal.textContent = els.windGust.value + "%";
}

let windTimer = null;
function sendWind() {
  send({
    cmd: "set_wind",
    speed_mps: Number(els.windSpeed.value),
    direction_deg: Number(els.windDir.value),
    gust: Number(els.windGust.value) / 100,
  });
}
function onWindInput() {
  refreshWindLabels();
  clearTimeout(windTimer);
  windTimer = setTimeout(sendWind, 120); // throttle
}
els.windSpeed.addEventListener("input", onWindInput);
els.windDir.addEventListener("input", onWindInput);
els.windGust.addEventListener("input", onWindInput);

function sendRepl(text) {
  const t = (text || "").trim();
  if (!t) return;
  send({ cmd: "send_mavlink", text: t });
}
els.replSend.addEventListener("click", () => {
  sendRepl(els.replInput.value);
  els.replInput.value = "";
});
els.replInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    sendRepl(els.replInput.value);
    els.replInput.value = "";
  }
});
for (const btn of document.querySelectorAll("[data-repl]")) {
  btn.addEventListener("click", () => sendRepl(btn.getAttribute("data-repl")));
}
els.logClear.addEventListener("click", () => {
  els.log.innerHTML = "";
});

// ---- pose-rate display ----------------------------------------------------
setInterval(() => {
  const now = performance.now();
  poseTimes = poseTimes.filter((t) => now - t < 1000);
  els.poseRate.textContent = String(poseTimes.length);
}, 500);

refreshWindLabels();
connect();
