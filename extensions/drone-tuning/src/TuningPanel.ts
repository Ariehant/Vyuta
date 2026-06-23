// Singleton webview panel hosting the Vyuta parameter tuning view.
//
// Phase 4: builds the tuning HTML (toolbar, snapshot bar, subsystem-grouped
// parameter tree, diff panel) with a strict CSP that allows the maestros
// WebSocket. The tree, editing, snapshots, and diffing live in the webview
// script (media/tuning.js); maestros owns the parameter store + MAVLink link.

import * as vscode from "vscode";

export class TuningPanel {
  public static readonly viewType = "vyuta.tuning";
  private static current: TuningPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;

    if (TuningPanel.current) {
      TuningPanel.current.panel.reveal(column);
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      TuningPanel.viewType,
      "Vyuta Parameters",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );

    TuningPanel.current = new TuningPanel(panel, extensionUri);
  }

  public static dispose(): void {
    TuningPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;

    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);

    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.tuning")) {
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
    const cfg = vscode.workspace.getConfiguration("vyuta.tuning");
    const config = {
      gatewayUrl: cfg.get<string>("gatewayUrl", "ws://127.0.0.1:9876"),
      liveTune: cfg.get<boolean>("liveTune", false),
    };

    const styleUri = this.mediaUri(webview, "tuning.css");
    const scriptUri = this.mediaUri(webview, "tuning.js");
    const nonce = getNonce();

    const csp = [
      `default-src 'none'`,
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
  <title>Vyuta Parameters</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Parameters</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span class="count">params <code id="param-count">0/0</code></span>
    <span class="count">modified <code id="modified-count">0</code></span>
  </header>

  <div class="toolbar">
    <input type="search" id="filter" placeholder="filter parameters…" />
    <label class="check"><input type="checkbox" id="live-tune" /> Live Tune</label>
    <button id="apply" class="btn btn--go" disabled>Apply</button>
    <button id="revert" class="btn" disabled>Revert</button>
    <span class="spacer"></span>
    <button id="refresh" class="btn btn--mini">Refresh</button>
    <button id="expand" class="btn btn--mini">Expand all</button>
    <button id="collapse" class="btn btn--mini">Collapse all</button>
  </div>

  <div class="toolbar toolbar--snap">
    <input type="text" id="snap-name" placeholder="snapshot name" />
    <button id="snap-save" class="btn btn--mini">Save snapshot</button>
    <span class="spacer"></span>
    <select id="snap-select"><option value="">— snapshot —</option></select>
    <button id="snap-diff" class="btn btn--mini">Diff</button>
    <button id="snap-delete" class="btn btn--mini">Delete</button>
    <button id="diff-clear" class="btn btn--mini hidden">Clear diff</button>
  </div>

  <div id="progress" class="progress hidden">
    <div id="progress-bar" class="progress-bar"></div>
    <span id="progress-label" class="progress-label"></span>
  </div>

  <div id="diff-panel" class="diff-panel hidden">
    <h2>Diff vs <code id="diff-name"></code></h2>
    <div id="diff-list" class="diff-list"></div>
  </div>

  <main id="tree" class="tree"></main>

  <footer class="vyuta-footer">
    <span>gateway: <code id="gateway-url"></code></span>
    <span id="hint">edits ${config.liveTune ? "apply live" : "are staged until Apply"}</span>
  </footer>

  <script nonce="${nonce}">
    window.__VYUTA_TUNING__ = ${JSON.stringify(config)};
  </script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    TuningPanel.current = undefined;
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
