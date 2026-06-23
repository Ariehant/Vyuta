// Vyuta companion (ROS 2) view (Phase 6).
//
// Connects to vyuta-agent, renders the ROS graph (nodes/topics/services),
// echoes a topic on click, drives colcon build / rsync deploy with a live log
// console, and asks the extension to open an SSH terminal. Auto-reconnects.
(function () {
  "use strict";

  const cfg = window.__VYUTA_COMPANION__ || {};
  const agentUrl = cfg.agentUrl || "ws://127.0.0.1:9879";
  const vscode = typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : null;

  const $ = (id) => document.getElementById(id);
  const els = {
    status: $("status"),
    rosBadge: $("ros-badge"),
    phase: $("phase"),
    build: $("build"),
    deploy: $("deploy"),
    cancel: $("cancel"),
    refresh: $("refresh"),
    ssh: $("ssh"),
    ws: $("ws"),
    target: $("target"),
    bridge: $("bridge"),
    message: $("message"),
    graphFilter: $("graph-filter"),
    nodes: $("nodes"),
    topics: $("topics"),
    services: $("services"),
    nNodes: $("n-nodes"),
    nTopics: $("n-topics"),
    nServices: $("n-services"),
    echo: $("echo"),
    echoTopic: $("echo-topic"),
    echoBody: $("echo-body"),
    echoClose: $("echo-close"),
    log: $("log"),
    logClear: $("log-clear"),
    agentUrl: $("agent-url"),
  };
  els.agentUrl.textContent = agentUrl;

  let ws = null;
  let reconnectDelay = 500;
  const MAX_DELAY = 8000;
  let graph = { nodes: [], topics: [], services: [] };

  const setStatus = (s, label) => {
    els.status.className = "status status--" + s;
    els.status.textContent = label;
  };
  const send = (o) => {
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(o));
  };

  function appendLog(stream, line) {
    const div = document.createElement("div");
    div.className = "log-line log-line--" + (stream || "agent");
    div.textContent = line;
    els.log.appendChild(div);
    while (els.log.childElementCount > 500) els.log.removeChild(els.log.firstChild);
    els.log.scrollTop = els.log.scrollHeight;
  }

  function renderGraph() {
    const filter = els.graphFilter.value.trim().toLowerCase();
    const match = (s) => !filter || s.toLowerCase().includes(filter);

    els.nodes.innerHTML = "";
    for (const n of graph.nodes.filter((x) => match(x.name))) {
      const row = document.createElement("div");
      row.className = "item";
      row.textContent = n.name;
      els.nodes.appendChild(row);
    }
    els.topics.innerHTML = "";
    for (const t of graph.topics.filter((x) => match(x.name) || match(x.type || ""))) {
      const row = document.createElement("button");
      row.className = "item item--btn";
      row.title = "click to echo one message\n" + (t.type || "");
      row.innerHTML = '<span class="item-name">' + t.name + "</span><span class=\"item-type\">" + (t.type || "") + "</span>";
      row.addEventListener("click", () => send({ cmd: "echo", topic: t.name }));
      els.topics.appendChild(row);
    }
    els.services.innerHTML = "";
    for (const s of graph.services.filter((x) => match(x.name) || match(x.type || ""))) {
      const row = document.createElement("div");
      row.className = "item";
      row.innerHTML = '<span class="item-name">' + s.name + "</span><span class=\"item-type\">" + (s.type || "") + "</span>";
      els.services.appendChild(row);
    }
    els.nNodes.textContent = String(graph.nodes.length);
    els.nTopics.textContent = String(graph.topics.length);
    els.nServices.textContent = String(graph.services.length);
  }

  function onStatus(m) {
    els.phase.textContent = m.phase;
    els.phase.className = "pill pill--" + m.phase;
    els.ws.textContent = m.workspace || "—";
    els.target.textContent = m.deploy_target || "(unset)";
    els.bridge.textContent = m.bridge || "—";
    els.message.textContent = m.message || "—";
    const busy = m.phase === "building" || m.phase === "deploying";
    els.build.disabled = busy;
    els.deploy.disabled = busy;
    els.cancel.disabled = !busy;
    els.rosBadge.classList.remove("hidden");
    els.rosBadge.textContent = m.ros_available ? "ROS 2" : "SYNTHETIC";
    els.rosBadge.className = "badge " + (m.ros_available ? "badge--ros" : "badge--synthetic");
  }

  function connect() {
    setStatus("connecting", "connecting…");
    try {
      ws = new WebSocket(agentUrl);
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
        case "graph":
          graph = { nodes: m.nodes || [], topics: m.topics || [], services: m.services || [] };
          renderGraph();
          break;
        case "status": onStatus(m); break;
        case "log": appendLog(m.stream, m.line); break;
        case "echo":
          els.echoTopic.textContent = m.topic;
          els.echoBody.textContent = m.sample;
          els.echo.classList.remove("hidden");
          break;
        case "ack": appendLog("agent", (m.ok ? "✓ " : "✗ ") + m.cmd + ": " + m.message); break;
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

  // controls
  const opt = (s) => (s ? s : undefined);
  els.build.addEventListener("click", () => send({ cmd: "build", workspace: opt(cfg.workspace) }));
  els.deploy.addEventListener("click", () =>
    send({ cmd: "deploy", source: opt(cfg.workspace), target: opt(cfg.deployTarget) })
  );
  els.cancel.addEventListener("click", () => send({ cmd: "cancel" }));
  els.refresh.addEventListener("click", () => send({ cmd: "graph" }));
  els.graphFilter.addEventListener("input", renderGraph);
  els.echoClose.addEventListener("click", () => els.echo.classList.add("hidden"));
  els.logClear.addEventListener("click", () => (els.log.innerHTML = ""));
  els.ssh.addEventListener("click", () => {
    if (vscode) vscode.postMessage({ type: "openSsh" });
  });

  connect();
})();
