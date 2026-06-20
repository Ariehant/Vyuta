// Singleton webview panel hosting the Vyuta telemetry cockpit.
//
// Phase 0 responsibilities:
//   * own the webview lifecycle (create / reveal / dispose)
//   * build a locked-down HTML document with a strict CSP that permits a
//     WebSocket connection to the configured gateway URL
//   * hand the webview the gateway URL so its client script can connect
//
// The actual connection + rendering happens in `media/main.js` inside the
// webview; the extension host side stays thin.

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

    // Re-render if the user changes the gateway URL setting.
    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.telemetry.gatewayUrl")) {
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

  private getHtml(webview: vscode.Webview): string {
    const gatewayUrl = vscode.workspace
      .getConfiguration("vyuta.telemetry")
      .get<string>("gatewayUrl", "ws://127.0.0.1:9876");

    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "media", "main.js")
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, "media", "main.css")
    );

    const nonce = getNonce();

    // Strict CSP. connect-src must allow the configured ws/wss gateway so the
    // webview client can open the telemetry socket.
    const csp = [
      `default-src 'none'`,
      `style-src ${webview.cspSource}`,
      `script-src 'nonce-${nonce}'`,
      `connect-src ws: wss:`,
      `img-src ${webview.cspSource} data:`,
    ].join("; ");

    return /* html */ `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link href="${styleUri}" rel="stylesheet" />
  <title>Vyuta Telemetry</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Telemetry</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span id="synthetic-badge" class="badge hidden">SYNTHETIC</span>
  </header>

  <main class="cockpit">
    <section class="card">
      <h2>Attitude</h2>
      <dl class="readout">
        <dt>Roll</dt><dd id="roll">—</dd>
        <dt>Pitch</dt><dd id="pitch">—</dd>
        <dt>Yaw</dt><dd id="yaw">—</dd>
      </dl>
    </section>
    <section class="card">
      <h2>Position</h2>
      <dl class="readout">
        <dt>Lat</dt><dd id="lat">—</dd>
        <dt>Lon</dt><dd id="lon">—</dd>
        <dt>Alt</dt><dd id="alt">—</dd>
      </dl>
    </section>
    <section class="card">
      <h2>Battery / Status</h2>
      <dl class="readout">
        <dt>Voltage</dt><dd id="battery_v">—</dd>
        <dt>Charge</dt><dd id="battery_pct">—</dd>
        <dt>Mode</dt><dd id="mode">—</dd>
        <dt>Armed</dt><dd id="armed">—</dd>
      </dl>
    </section>
  </main>

  <footer class="vyuta-footer">
    <span>gateway: <code id="gateway-url"></code></span>
    <span>frames: <code id="frame-count">0</code></span>
    <span class="phase-note">Phase 0 scaffold — Three.js attitude + Leaflet map arrive in Phase 1.</span>
  </footer>

  <script nonce="${nonce}">
    window.__VYUTA_GATEWAY_URL__ = ${JSON.stringify(gatewayUrl)};
  </script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
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
