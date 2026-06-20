// Vyuta Drone Telemetry — extension entry point (Phase 0 scaffold).
//
// Registers the "Vyuta: Open Telemetry Panel" command, which opens a webview
// that connects to the `maestros` telemetry gateway sidecar over WebSocket and
// renders a minimal live readout. Phase 1 grows this panel into the full
// Three.js attitude indicator + Leaflet map cockpit.

import * as vscode from "vscode";
import { TelemetryPanel } from "./TelemetryPanel";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.openTelemetryPanel", () => {
      TelemetryPanel.createOrShow(context.extensionUri);
    })
  );
}

export function deactivate(): void {
  TelemetryPanel.dispose();
}
