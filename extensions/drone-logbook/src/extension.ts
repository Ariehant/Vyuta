// Vyuta Drone Flight Log Analyzer — extension entry point (Phase 5).
//
// Registers the "Vyuta: Open Flight Log Analyzer" command, which opens a webview
// that talks to the `logbook` sidecar over WebSocket: load a PX4 ULog, browse a
// mode-annotated timeline, plot any field, and review an auto-generated checklist.

import * as vscode from "vscode";
import { LogbookPanel } from "./LogbookPanel";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.logbook.openPanel", () => {
      LogbookPanel.createOrShow(context.extensionUri);
    })
  );
}

export function deactivate(): void {
  LogbookPanel.dispose();
}
