// Singleton webview panel for the Vyuta companion (ROS 2) view.
//
// Phase 6: a mini-rqt graph browser (nodes / topics / services), colcon build &
// deploy controls with a log console, and an "SSH" button that asks the
// extension host to open an integrated SSH terminal. Talks to vyuta-agent over
// WebSocket; a strict CSP allows that connection.

import * as vscode from "vscode";
import { openSshTerminal } from "./extension";

export class CompanionPanel {
  public static readonly viewType = "vyuta.companion";
  private static current: CompanionPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private disposables: vscode.Disposable[] = [];

  public static createOrShow(extensionUri: vscode.Uri): void {
    const column = vscode.window.activeTextEditor?.viewColumn;
    if (CompanionPanel.current) {
      CompanionPanel.current.panel.reveal(column);
      return;
    }
    const panel = vscode.window.createWebviewPanel(
      CompanionPanel.viewType,
      "Vyuta Companion",
      column ?? vscode.ViewColumn.One,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [vscode.Uri.joinPath(extensionUri, "media")],
      }
    );
    CompanionPanel.current = new CompanionPanel(panel, extensionUri);
  }

  public static dispose(): void {
    CompanionPanel.current?.panel.dispose();
  }

  private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.render();
    this.panel.onDidDispose(() => this.onDispose(), null, this.disposables);
    this.panel.webview.onDidReceiveMessage(
      (msg) => {
        if (msg?.type === "openSsh") {
          openSshTerminal();
        }
      },
      null,
      this.disposables
    );
    vscode.workspace.onDidChangeConfiguration(
      (e) => {
        if (e.affectsConfiguration("vyuta.companion")) {
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
    const cfg = vscode.workspace.getConfiguration("vyuta.companion");
    const config = {
      agentUrl: cfg.get<string>("agentUrl", "ws://127.0.0.1:9879"),
      sshHost: cfg.get<string>("sshHost", ""),
      workspace: cfg.get<string>("workspace", ""),
      deployTarget: cfg.get<string>("deployTarget", ""),
    };

    const styleUri = this.mediaUri(webview, "companion.css");
    const scriptUri = this.mediaUri(webview, "companion.js");
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
  <title>Vyuta Companion</title>
</head>
<body>
  <header class="vyuta-header">
    <h1>Vyuta Companion</h1>
    <span id="status" class="status status--connecting">connecting…</span>
    <span id="ros-badge" class="badge hidden"></span>
    <span id="phase" class="pill">idle</span>
  </header>

  <div class="toolbar">
    <button id="build" class="btn btn--go">⚙ Build</button>
    <button id="deploy" class="btn btn--go">⇪ Deploy to Drone</button>
    <button id="cancel" class="btn btn--stop" disabled>■ Cancel</button>
    <button id="refresh" class="btn btn--mini">⟳ Refresh graph</button>
    <span class="spacer"></span>
    <button id="ssh" class="btn btn--mini">⌨ SSH</button>
  </div>

  <dl class="readout">
    <dt>Workspace</dt><dd id="ws">—</dd>
    <dt>Deploy target</dt><dd id="target">—</dd>
    <dt>Bridge</dt><dd id="bridge">—</dd>
    <dt>Message</dt><dd id="message">—</dd>
  </dl>

  <main class="body">
    <section class="graph">
      <div class="graph-head">
        <h2>ROS 2 graph</h2>
        <input type="search" id="graph-filter" placeholder="filter…" />
      </div>
      <div class="cols">
        <div class="col"><h3>Nodes <span id="n-nodes" class="c">0</span></h3><div id="nodes" class="list"></div></div>
        <div class="col"><h3>Topics <span id="n-topics" class="c">0</span></h3><div id="topics" class="list"></div></div>
        <div class="col"><h3>Services <span id="n-services" class="c">0</span></h3><div id="services" class="list"></div></div>
      </div>
      <div id="echo" class="echo hidden"><div class="echo-head"><b id="echo-topic"></b><button id="echo-close" class="btn btn--mini">close</button></div><pre id="echo-body"></pre></div>
    </section>

    <section class="logs">
      <h2>Log <button id="log-clear" class="btn btn--mini">clear</button></h2>
      <div id="log" class="log" role="log"></div>
    </section>
  </main>

  <footer class="vyuta-footer">
    <span>agent: <code id="agent-url"></code></span>
  </footer>

  <script nonce="${nonce}">window.__VYUTA_COMPANION__ = ${JSON.stringify(config)};</script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private onDispose(): void {
    CompanionPanel.current = undefined;
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
