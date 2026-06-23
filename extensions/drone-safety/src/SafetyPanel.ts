// Singleton webview panel for the Vyuta pre-flight & safety view (Phase 7).
//
// Runs the maestros pre-flight checklist, gates an Arm button on it, and offers
// Disarm. Talks to maestros over WebSocket; a strict CSP allows that link.

import * as vscode from "vscode";

export class SafetyPanel {
  public static readonly viewType = "vyuta.safety";
  private static current: SafetyPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;
    if (SafetyPanel.current) {
      SafetyPanel.current.panel.reveal(column);
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      SafetyPanel.viewType,
      "Vyuta Pre-Flight",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );
    SafetyPanel.current = new SafetyPanel(panel, extensionUri);
  }

  public static dispose(): void {
    SafetyPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);
    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.safety")) {
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
    return webview.asWebviewUri(vscode.Uri.joinPath(this.extensionUri, "media", ...path));
  }

  private getHtml(webview: vscode.Webview): string {
    const cfg = vscode.workspace.getConfiguration("vyuta.safety");
    const config = {
      gatewayUrl: cfg.get<string>("gatewayUrl", "ws://127.0.0.1:9876"),
      audibleAlarms: cfg.get<boolean>("audibleAlarms", true),
    };
    const styleUri = this.mediaUri(webview, "safety.css");
    const scriptUri = this.mediaUri(webview, "safety.js");
    const nonce = getNonce();
    const csp = [
      `default-src 'none'`,
      `style-src ${webview.cspSource} 'unsafe-inline'`,
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
  <title>Vyuta Pre-Flight</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Pre-Flight</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span id="armed" class="pill pill--armed hidden">ARMED</span>
  </header>

  <div id="banner" class="banner hidden"></div>

  <section class="checklist-card">
    <h2>Pre-flight checklist <button id="refresh" class="btn btn--mini">recheck</button></h2>
    <div id="checklist" class="checklist"></div>
  </section>

  <div class="arm-row">
    <button id="arm" class="arm-btn" disabled>ARM</button>
    <button id="disarm" class="disarm-btn" disabled>DISARM</button>
  </div>
  <p id="arm-msg" class="arm-msg"></p>

  <footer class="vyuta-footer">
    <span>gateway: <code id="gateway-url"></code></span>
  </footer>

  <script nonce="${nonce}">window.__VYUTA_SAFETY__ = ${JSON.stringify(config)};</script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    SafetyPanel.current = undefined;
    while (this.disposables.length) {
      this.disposables.pop()?.dispose();
    }
  }
}

function getNonce(): string {
  let text = "";
  const possible = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  for (let i = 0; i < 32; i++) {
    text += possible.charAt(Math.floor(Math.random() * possible.length));
  }
  return text;
}
