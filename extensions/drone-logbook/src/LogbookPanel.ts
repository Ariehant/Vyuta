// Singleton webview panel hosting the Vyuta flight-log analyzer.
//
// Phase 5: builds the analyzer HTML (overview, auto-review checklist, mode
// timeline, field picker, stacked uPlot charts, logged-message list) with a
// strict CSP that allows the logbook WebSocket and the vendored uPlot library.
// The browsing/plotting logic lives in media/logbook.js.

import * as vscode from "vscode";

export class LogbookPanel {
  public static readonly viewType = "vyuta.logbook";
  private static current: LogbookPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;
    if (LogbookPanel.current) {
      LogbookPanel.current.panel.reveal(column);
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      LogbookPanel.viewType,
      "Vyuta Flight Log",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );
    LogbookPanel.current = new LogbookPanel(panel, extensionUri);
  }

  public static dispose(): void {
    LogbookPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);
    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.logbook")) {
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
    const cfg = vscode.workspace.getConfiguration("vyuta.logbook");
    const config = { serverUrl: cfg.get<string>("serverUrl", "ws://127.0.0.1:9878") };

    const styleUri = this.mediaUri(webview, "logbook.css");
    const scriptUri = this.mediaUri(webview, "logbook.js");
    const uplotCss = this.mediaUri(webview, "vendor", "uplot", "uPlot.min.css");
    const uplotJs = this.mediaUri(webview, "vendor", "uplot", "uPlot.iife.min.js");
    const nonce = getNonce();

    const csp = [
      `default-src 'none'`,
      `img-src ${webview.cspSource} data:`,
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
  <link href="${uplotCss}" rel="stylesheet" />
  <link href="${styleUri}" rel="stylesheet" />
  <title>Vyuta Flight Log</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Flight Log</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span class="count"><code id="log-name">—</code></span>
    <span class="count">dur <code id="log-dur">—</code></span>
  </header>

  <div class="toolbar">
    <input type="text" id="path" placeholder="/path/to/flight.ulg" />
    <button id="load" class="btn btn--mini">Load .ulg</button>
    <button id="synthetic" class="btn btn--mini">Synthetic</button>
    <span class="spacer"></span>
    <span class="count" id="source">—</span>
  </div>

  <section class="card">
    <h2>Auto-review</h2>
    <div id="review" class="review"></div>
  </section>

  <section class="card">
    <h2>Flight modes</h2>
    <div id="timeline" class="timeline"></div>
    <div id="timeline-legend" class="legend"></div>
  </section>

  <main class="body">
    <aside class="picker">
      <h2>Fields <input type="search" id="field-filter" placeholder="filter…" /></h2>
      <div id="fields" class="fields"></div>
    </aside>
    <section class="charts-wrap">
      <div id="charts" class="charts"></div>
      <p id="charts-hint" class="hint">Select fields on the left to plot them.</p>
    </section>
  </main>

  <section class="card">
    <h2>Logged messages</h2>
    <div id="messages" class="messages"></div>
  </section>

  <footer class="vyuta-footer">
    <span>sidecar: <code id="server-url"></code></span>
  </footer>

  <script nonce="${nonce}">window.__VYUTA_LOG__ = ${JSON.stringify(config)};</script>
  <script nonce="${nonce}" src="${uplotJs}"></script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    LogbookPanel.current = undefined;
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
