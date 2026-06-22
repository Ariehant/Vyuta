// Vyuta Drone Firmware — extension entry point (Phase 2).
//
// Registers the PX4 build-task provider, the `vyuta-probe-rs` debug adapter
// factory, and the build/flash/probe/RTT commands.

import * as vscode from "vscode";
import { Px4TaskProvider } from "./taskProvider";
import { ProbeRsDebugAdapterFactory } from "./debugAdapter";
import { buildFirmware, flashFirmware, listProbes, openRttTerminal } from "./commands";

export function activate(context: vscode.ExtensionContext): void {
  const debugFactory = new ProbeRsDebugAdapterFactory();

  context.subscriptions.push(
    vscode.tasks.registerTaskProvider(Px4TaskProvider.TYPE, new Px4TaskProvider()),
    vscode.debug.registerDebugAdapterDescriptorFactory("vyuta-probe-rs", debugFactory),
    debugFactory,
    vscode.commands.registerCommand("vyuta.firmware.build", buildFirmware),
    vscode.commands.registerCommand("vyuta.firmware.flash", flashFirmware),
    vscode.commands.registerCommand("vyuta.firmware.listProbes", listProbes),
    vscode.commands.registerCommand("vyuta.firmware.openRttTerminal", openRttTerminal)
  );
}

export function deactivate(): void {
  /* subscriptions disposed by VS Code */
}
