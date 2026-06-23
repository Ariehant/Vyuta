// Vyuta Drone Companion — extension entry point (Phase 6).
//
// Registers the "Vyuta: Open Companion (ROS 2) Panel" command (a webview talking
// to the vyuta-agent over WebSocket) and a "Open Drone SSH Terminal" command
// that opens an integrated terminal SSH'd to the companion computer.

import * as vscode from "vscode";
import { CompanionPanel } from "./CompanionPanel";

export function openSshTerminal(): void {
  const host = vscode.workspace.getConfiguration("vyuta.companion").get<string>("sshHost", "");
  if (!host) {
    vscode.window.showWarningMessage(
      "Vyuta: set vyuta.companion.sshHost (e.g. pi@drone.local) to open an SSH terminal."
    );
    return;
  }
  const terminal = vscode.window.createTerminal({ name: `Drone SSH (${host})` });
  terminal.show();
  terminal.sendText(`ssh ${host}`);
}

export function activate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("vyuta.companion.openPanel", () => {
      CompanionPanel.createOrShow(context.extensionUri);
    }),
    vscode.commands.registerCommand("vyuta.companion.openSshTerminal", openSshTerminal)
  );
}

export function deactivate(): void {
  CompanionPanel.dispose();
}
