// Vyuta Drone Pre-Flight & Safety — entry point (Phase 7).
//
// Registers "Vyuta: Open Pre-Flight & Safety Panel": a webview that runs the
// maestros pre-flight checklist, gates the Arm button on it, and disarms.

import * as vscode from "vscode";
import { SafetyPanel } from "./SafetyPanel";

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.safety.openPanel", () => {
      SafetyPanel.createOrShow(context.extensionUri);
    })
  );
}

export function deactivate(): void {
  SafetyPanel.dispose();
}
