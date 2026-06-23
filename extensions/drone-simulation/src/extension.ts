// Vyuta Drone Simulation — extension entry point (Phase 3).
//
// Registers the "Vyuta: Open Simulation Control Panel" command, which opens a
// webview that talks to the `sim-manager` sidecar over WebSocket: start/stop a
// PX4-SITL + Gazebo simulation, monitor status/logs, inject wind, drive a
// mission REPL, and watch the vehicle fly in an embedded Three.js 3D viewport.

import * as vscode from "vscode";
import { SimulationPanel } from "./SimulationPanel";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.simulation.openPanel", () => {
      SimulationPanel.createOrShow(context.extensionUri);
    })
  );
}

export function deactivate(): void {
  SimulationPanel.dispose();
}
