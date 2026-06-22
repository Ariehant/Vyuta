// Firmware commands: build, flash, list probes, RTT/semihosting terminal.

import * as vscode from "vscode";
import { loadProbeAddon } from "./probeAddon";

/** Build firmware by picking one of the provided `px4` tasks. */
export async function buildFirmware(): Promise<void> {
  const tasks = (await vscode.tasks.fetchTasks({ type: "px4" })) ?? [];
  if (tasks.length === 0) {
    vscode.window.showWarningMessage("Vyuta: no PX4 build tasks available.");
    return;
  }
  const pick = await vscode.window.showQuickPick(
    tasks.map((t) => ({ label: t.name, detail: t.detail, task: t })),
    { placeHolder: "Select a PX4 build target" }
  );
  if (pick) {
    await vscode.tasks.executeTask(pick.task);
  }
}

/** Flash firmware via the PX4 uploader, probe-rs, or dfu-util. */
export async function flashFirmware(): Promise<void> {
  const cfg = vscode.workspace.getConfiguration("vyuta.firmware");
  const method = await vscode.window.showQuickPick(
    [
      { label: "PX4 make upload", id: "make", detail: "make <target> upload (USB/serial bootloader)" },
      { label: "probe-rs", id: "probe-rs", detail: "probe-rs download --chip <chip> <elf>" },
      { label: "dfu-util", id: "dfu", detail: "dfu-util DFU flashing" },
    ],
    { placeHolder: "Select a flashing method" }
  );
  if (!method) return;

  if (method.id === "make") {
    const target = await vscode.window.showInputBox({
      prompt: "PX4 make target to upload",
      value: "px4_fmu-v6x_default",
    });
    if (!target) return;
    await runInTerminal("Vyuta Flash", `make ${target} upload`, cfg.get<string>("px4Dir"));
  } else if (method.id === "probe-rs") {
    const chip = await vscode.window.showInputBox({ prompt: "Target chip (probe-rs)", value: "STM32H743ZITx" });
    if (!chip) return;
    const elf = await pickElf();
    if (!elf) return;
    const probeRs = cfg.get<string>("probeRsPath", "probe-rs");
    await runInTerminal("Vyuta Flash", `${probeRs} download --chip ${chip} "${elf}"`);
  } else {
    const elf = await pickElf("bin");
    if (!elf) return;
    const dfu = cfg.get<string>("dfuUtilPath", "dfu-util");
    await runInTerminal("Vyuta Flash", `${dfu} -a 0 -s 0x08000000:leave -D "${elf}"`);
  }
}

/** Show attached debug probes (via the in-process probe-rs addon). */
export async function listProbes(): Promise<void> {
  const addon = loadProbeAddon();
  if (!addon) {
    vscode.window.showWarningMessage(
      "Vyuta: probe-rs addon not built. Run `npm run build` in rust/probe-rs-extension."
    );
    return;
  }
  const probes = addon.listProbes();
  if (probes.length === 0) {
    vscode.window.showInformationMessage("Vyuta: no debug probes detected.");
    return;
  }
  await vscode.window.showQuickPick(
    probes.map((p) => ({
      label: p.identifier,
      detail: `VID:PID ${hex(p.vendorId)}:${hex(p.productId)}${
        p.serialNumber ? ` · serial ${p.serialNumber}` : ""
      }`,
    })),
    { placeHolder: `${probes.length} probe(s) detected` }
  );
}

/** Open a terminal attached to the target's RTT / semihosting output. */
export async function openRttTerminal(): Promise<void> {
  const cfg = vscode.workspace.getConfiguration("vyuta.firmware");
  const chip = await vscode.window.showInputBox({
    prompt: "Target chip for RTT attach",
    value: "STM32H743ZITx",
  });
  if (!chip) return;
  const probeRs = cfg.get<string>("probeRsPath", "probe-rs");
  await runInTerminal("Vyuta RTT", `${probeRs} attach --chip ${chip}`);
}

// --- helpers ---------------------------------------------------------------

function hex(n: number): string {
  return "0x" + n.toString(16).padStart(4, "0");
}

async function pickElf(ext = "elf"): Promise<string | undefined> {
  const uris = await vscode.window.showOpenDialog({
    canSelectMany: false,
    openLabel: "Select firmware",
    filters: { Firmware: [ext], "All files": ["*"] },
  });
  return uris?.[0]?.fsPath;
}

async function runInTerminal(name: string, command: string, cwd?: string): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  const resolvedCwd = cwd?.replace(/\$\{workspaceFolder\}/g, folder ?? "") || folder;
  const terminal = vscode.window.createTerminal({ name, cwd: resolvedCwd });
  terminal.show();
  terminal.sendText(command);
}
