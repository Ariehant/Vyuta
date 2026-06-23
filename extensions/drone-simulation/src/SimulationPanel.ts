// Singleton webview panel hosting the Vyuta simulation control cockpit.
//
// Phase 3: builds the control HTML (world/vehicle pickers, start/stop/reset,
// wind injection, mission REPL, log console) and an embedded Three.js 3D
// viewport. A strict CSP allows the sim-manager WebSocket and the vendored
// Three.js module; the control logic + scene live in the webview ES modules
// (media/sim.js, media/viewport3d.js).

import * as vscode from "vscode";

export class SimulationPanel {
  public static readonly viewType = "vyuta.simulation";
  private static current: SimulationPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;

    if (SimulationPanel.current) {
      SimulationPanel.current.panel.reveal(column);
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      SimulationPanel.viewType,
      "Vyuta Simulation",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );

    SimulationPanel.current = new SimulationPanel(panel, extensionUri);
  }

  public static dispose(): void {
    SimulationPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;

    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);

    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.simulation")) {
          this.render();
        }
      },
      null,
      this.disposables
    );
  }

  private render(): void {
    this.panel.webview.html = this.getHtml(this.panel.webview);
  }

  private mediaUri(webview: vscode.Webview, ...path: string[]): vscode.Uri {
    return webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "media", ...path)
    );
  }

  private getHtml(webview: vscode.Webview): string {
    const cfg = vscode.workspace.getConfiguration("vyuta.simulation");
    const config = {
      managerUrl: cfg.get<string>("managerUrl", "ws://127.0.0.1:9877"),
      defaultWorld: cfg.get<string>("defaultWorld", "default"),
      defaultVehicle: cfg.get<string>("defaultVehicle", "x500"),
      headless: cfg.get<boolean>("headless", true),
      forceMock: cfg.get<boolean>("forceMock", false),
    };

    const styleUri = this.mediaUri(webview, "sim.css");
    const simUri = this.mediaUri(webview, "sim.js");
    const threeUri = this.mediaUri(webview, "vendor", "three", "three.module.min.js");

    const nonce = getNonce();

    // Three.js (WebGL) needs blob:/data: for buffers/canvas captures; styles are
    // partly inline. Scripts: vendored module from cspSource + nonce'd inline.
    const csp = [
      `default-src 'none'`,
      `img-src ${webview.cspSource} data: blob:`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `font-src ${webview.cspSource}`,
      `script-src ${webview.cspSource} 'nonce-${nonce}'`,
      `connect-src ws: wss:`,
    ].join("; ");

    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link href="${styleUri}" rel="stylesheet" />
  <title>Vyuta Simulation</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Simulation</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span id="mock-badge" class="badge badge--mock hidden">MOCK</span>
    <span id="phase" class="pill">idle</span>
    <span id="flight-mode" class="pill">—</span>
    <span id="armed" class="pill pill--armed hidden">ARMED</span>
  </header>

  <main class="sim">
    <section class="col col--controls">
      <div class="card">
        <h2>Simulation</h2>
        <label class="field">
          <span>World</span>
          <select id="world"></select>
        </label>
        <label class="field">
          <span>Vehicle <code id="vehicle-class" class="vclass"></code></span>
          <select id="vehicle"></select>
        </label>
        <label class="field">
          <span>Simulator</span>
          <select id="simulator"></select>
        </label>
        <label class="check"><input type="checkbox" id="headless" /> Headless (no Gazebo GUI)</label>
        <label class="check"><input type="checkbox" id="force-mock" /> Force mock flight</label>
        <div class="btn-row">
          <button id="btn-start" class="btn btn--go">▶ Start</button>
          <button id="btn-stop" class="btn btn--stop" disabled>■ Stop</button>
          <button id="btn-reset" class="btn">↺ Reset</button>
        </div>
        <dl class="readout">
          <dt>Simulator</dt><dd id="sim-backend">—</dd>
          <dt>Target</dt><dd id="make-target">—</dd>
          <dt>PID</dt><dd id="pid">—</dd>
          <dt>Sim time</dt><dd id="sim-time">—</dd>
          <dt>Message</dt><dd id="message">—</dd>
        </dl>
      </div>

      <div class="card">
        <h2>Wind</h2>
        <label class="slider">
          <span>Speed <code id="wind-speed-val">0.0 m/s</code></span>
          <input type="range" id="wind-speed" min="0" max="20" step="0.5" value="0" />
        </label>
        <label class="slider">
          <span>Direction <code id="wind-dir-val">0°</code></span>
          <input type="range" id="wind-dir" min="0" max="359" step="1" value="0" />
        </label>
        <label class="slider">
          <span>Gust <code id="wind-gust-val">0%</code></span>
          <input type="range" id="wind-gust" min="0" max="100" step="1" value="0" />
        </label>
      </div>

      <div class="card">
        <h2>Mission REPL</h2>
        <div class="btn-row btn-row--wrap">
          <button class="btn btn--mini" data-repl="arm">arm</button>
          <button class="btn btn--mini" data-repl="takeoff 5">takeoff</button>
          <button class="btn btn--mini" data-repl="hold">hold</button>
          <button class="btn btn--mini" data-repl="orbit 8 6">orbit</button>
          <button class="btn btn--mini" data-repl="rtl">rtl</button>
          <button class="btn btn--mini" data-repl="land">land</button>
        </div>
        <div class="repl">
          <input type="text" id="repl-input" placeholder="e.g. goto 20 0 6" />
          <button id="repl-send" class="btn">Send</button>
        </div>
        <p class="hint">Verbs: arm · disarm · takeoff [alt] · hold · goto x y [z] · orbit [r] [alt] · rtl · land</p>
      </div>
    </section>

    <section class="col col--viewport">
      <div class="card card--viewport">
        <h2>3D Viewport</h2>
        <div class="viewport-wrap">
          <canvas id="viewport"></canvas>
          <div class="overlay">
            <span>X <b id="ov-x">—</b></span>
            <span>Y <b id="ov-y">—</b></span>
            <span>Alt <b id="ov-z">—</b></span>
            <span>Spd <b id="ov-spd">—</b></span>
          </div>
          <div class="viewport-hint">drag to orbit · scroll to zoom</div>
        </div>
      </div>
    </section>
  </main>

  <section class="card card--log">
    <h2>Log <button id="log-clear" class="btn btn--mini">clear</button></h2>
    <div id="log" class="log" role="log"></div>
  </section>

  <footer class="vyuta-footer">
    <span>sidecar: <code id="manager-url"></code></span>
    <span>pose: <code id="pose-rate">0</code> Hz</span>
  </footer>

  <script type="importmap" nonce="${nonce}">
  { "imports": { "three": "${threeUri}" } }
  </script>
  <script nonce="${nonce}">
    window.__VYUTA_SIM__ = ${JSON.stringify(config)};
  </script>
  <script type="module" nonce="${nonce}" src="${simUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    SimulationPanel.current = undefined;
    while (this.disposables.length) {
      this.disposables.pop()?.dispose();
    }
  }
}

function getNonce(): string {
  let text = "";
  const possible =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
