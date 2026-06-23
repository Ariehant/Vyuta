// Vyuta Drone Parameter Tuning — extension entry point (Phase 4).
//
// Registers the "Vyuta: Open Parameter Tuning Panel" command, which opens a
// webview that talks to the maestros gateway over WebSocket: fetch the vehicle
// parameter list, edit values (live or staged), and save / diff snapshots.

import * as vscode from "vscode";
import { TuningPanel } from "./TuningPanel";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.tuning.openPanel", () => {
      TuningPanel.createOrShow(context.extensionUri);
    })
  );
}

export function deactivate(): void {
  TuningPanel.dispose();
}
