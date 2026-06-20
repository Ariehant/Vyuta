// Singleton webview panel hosting the Vyuta telemetry cockpit.
//
// Phase 1: builds the cockpit HTML (artificial horizon canvas, Leaflet GPS
// map, battery gauge, status + alarm banner), wires a strict CSP that allows
// the telemetry WebSocket and OpenStreetMap tiles, and injects the runtime
// configuration. Rendering + the socket live in the webview scripts.

import * as vscode from "vscode";

export class TelemetryPanel {
  public static readonly viewType = "vyuta.telemetry";
  private static current: TelemetryPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;

    if (TelemetryPanel.current) {
      TelemetryPanel.current.panel.reveal(column);
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      TelemetryPanel.viewType,
      "Vyuta Telemetry",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );

    TelemetryPanel.current = new TelemetryPanel(panel, extensionUri);
  }

  public static dispose(): void {
    TelemetryPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;

    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);

    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.telemetry")) {
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
    const cfg = vscode.workspace.getConfiguration("vyuta.telemetry");
    const config = {
      gatewayUrl: cfg.get<string>("gatewayUrl", "ws://127.0.0.1:9876"),
      batteryWarnPercent: cfg.get<number>("batteryWarnPercent", 25),
      batteryCriticalPercent: cfg.get<number>("batteryCriticalPercent", 15),
      audibleAlarms: cfg.get<boolean>("audibleAlarms", true),
    };

    const leafletCss = this.mediaUri(webview, "vendor", "leaflet", "leaflet.css");
    const leafletJs = this.mediaUri(webview, "vendor", "leaflet", "leaflet.js");
    const styleUri = this.mediaUri(webview, "main.css");
    const attitudeUri = this.mediaUri(webview, "attitude.js");
    const mapUri = this.mediaUri(webview, "map.js");
    const cockpitUri = this.mediaUri(webview, "cockpit.js");

    const nonce = getNonce();

    // OpenStreetMap raster tiles are loaded as <img>, so they need img-src.
    // Leaflet sets element style attributes, requiring 'unsafe-inline' styles.
    const csp = [
      `default-src 'none'`,
      `img-src ${webview.cspSource} data: blob: https://*.tile.openstreetmap.org`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
      `script-src 'nonce-${nonce}'`,
      `connect-src ws: wss:`,
    ].join("; ");

    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link href="${leafletCss}" rel="stylesheet" />
  <link href="${styleUri}" rel="stylesheet" />
  <title>Vyuta Telemetry</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Telemetry</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span id="source-badge" class="badge hidden"></span>
    <span id="mode" class="pill">—</span>
    <span id="armed" class="pill pill--armed hidden">ARMED</span>
  </header>

  <div id="alarm" class="alarm hidden" role="alert"></div>

  <main class="cockpit">
    <section class="card card--horizon">
      <h2>Attitude</h2>
      <div class="horizon-wrap"><canvas id="horizon"></canvas></div>
      <dl class="readout readout--inline">
        <dt>Roll</dt><dd id="roll">—</dd>
        <dt>Pitch</dt><dd id="pitch">—</dd>
        <dt>Hdg</dt><dd id="heading">—</dd>
      </dl>
    </section>

    <section class="card card--map">
      <h2>Position</h2>
      <div id="map"></div>
      <dl class="readout readout--inline">
        <dt>Lat</dt><dd id="lat">—</dd>
        <dt>Lon</dt><dd id="lon">—</dd>
        <dt>Alt</dt><dd id="alt">—</dd>
      </dl>
    </section>

    <section class="card">
      <h2>Battery</h2>
      <div class="gauge"><div id="battery-fill" class="gauge-fill"></div><span id="battery-pct-label" class="gauge-label">—</span></div>
      <dl class="readout">
        <dt>Voltage</dt><dd id="battery_v">—</dd>
        <dt>Current</dt><dd id="current">—</dd>
      </dl>
    </section>

    <section class="card">
      <h2>Air data / Link</h2>
      <dl class="readout">
        <dt>Groundspeed</dt><dd id="groundspeed">—</dd>
        <dt>Airspeed</dt><dd id="airspeed">—</dd>
        <dt>Climb</dt><dd id="climb">—</dd>
        <dt>Throttle</dt><dd id="throttle">—</dd>
        <dt>Status</dt><dd id="system_status">—</dd>
        <dt>Link</dt><dd id="link">—</dd>
      </dl>
    </section>
  </main>

  <footer class="vyuta-footer">
    <span>gateway: <code id="gateway-url"></code></span>
    <span>frames: <code id="frame-count">0</code></span>
    <span>rate: <code id="frame-rate">0</code> Hz</span>
  </footer>

  <script nonce="${nonce}">
    window.__VYUTA__ = ${JSON.stringify(config)};
  </script>
  <script nonce="${nonce}" src="${leafletJs}"></script>
  <script nonce="${nonce}" src="${attitudeUri}"></script>
  <script nonce="${nonce}" src="${mapUri}"></script>
  <script nonce="${nonce}" src="${cockpitUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    TelemetryPanel.current = undefined;
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
