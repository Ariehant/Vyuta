// Controller for `.mission` notebooks (Phase 7).
//
// Parses each code cell's mission commands, validates them, and flies them on
// the sim-manager sidecar (whose 3D viewport shows the result in real time).
// If no WebSocket is available in the extension host it falls back to a dry-run
// that validates and prints the plan.

import * as vscode from "vscode";

const VERBS: Record<string, [number, number]> = {
  // verb: [minArgs, maxArgs]
  arm: [0, 0],
  disarm: [0, 0],
  takeoff: [0, 1],
  goto: [2, 3],
  hold: [0, 0],
  orbit: [0, 2],
  rtl: [0, 0],
  land: [0, 0],
  wait: [1, 1],
};

interface Step {
  raw: string;
  verb: string;
  args: number[];
  ok: boolean;
  error?: string;
}

function parseMission(text: string): Step[] {
  const steps: Step[] = [];
  for (const line of text.split(/\r?\n/)) {
    const t = line.trim();
    if (!t || t.startsWith("#")) continue;
    const parts = t.split(/\s+/);
    const verb = parts[0].toLowerCase();
    const rest = parts.slice(1);
    const spec = VERBS[verb];
    if (!spec) {
      steps.push({ raw: t, verb, args: [], ok: false, error: "unknown command" });
      continue;
    }
    const nums = rest.map(Number);
    if (nums.some((n) => Number.isNaN(n))) {
      steps.push({ raw: t, verb, args: [], ok: false, error: "non-numeric argument" });
      continue;
    }
    if (nums.length < spec[0] || nums.length > spec[1]) {
      steps.push({ raw: t, verb, args: nums, ok: false, error: `expects ${spec[0]}–${spec[1]} args` });
      continue;
    }
    steps.push({ raw: t, verb, args: nums, ok: true });
  }
  return steps;
}

export class MissionController {
  public readonly controller: vscode.NotebookController;

  constructor() {
    this.controller = vscode.notebooks.createNotebookController(
      "vyuta-mission-runner",
      "vyuta-mission",
      "Vyuta Simulation"
    );
    this.controller.supportedLanguages = ["plaintext"];
    this.controller.supportsExecutionOrder = true;
    this.controller.description = "Fly the mission on the Vyuta simulation";
    this.controller.executeHandler = this.execute.bind(this);
  }

  dispose(): void {
    this.controller.dispose();
  }

  private async execute(
    cells: vscode.NotebookCell[],
    _notebook: vscode.NotebookDocument,
    controller: vscode.NotebookController
  ): Promise<void> {
    for (const cell of cells) {
      await this.runCell(cell, controller);
    }
  }

  private async runCell(cell: vscode.NotebookCell, controller: vscode.NotebookController): Promise<void> {
    const exec = controller.createNotebookCellExecution(cell);
    exec.start(Date.now());
    const lines: string[] = [];
    const flush = async () => {
      await exec.replaceOutput([
        new vscode.NotebookCellOutput([vscode.NotebookCellOutputItem.text(lines.join("\n"))]),
      ]);
    };

    const steps = parseMission(cell.document.getText());
    const invalid = steps.filter((s) => !s.ok);
    if (steps.length === 0) {
      lines.push("(no commands)");
      await flush();
      exec.end(true, Date.now());
      return;
    }
    if (invalid.length) {
      lines.push("⚠ validation errors:");
      for (const s of invalid) lines.push(`  ✗ ${s.raw} — ${s.error}`);
      lines.push("");
    }

    const cfg = vscode.workspace.getConfiguration("vyuta.mission");
    const simUrl = cfg.get<string>("simUrl", "ws://127.0.0.1:9877");
    const pause = cfg.get<number>("stepPauseMs", 600);
    const valid = steps.filter((s) => s.ok);

    const WS: any = (globalThis as any).WebSocket;
    let cancelled = false;
    exec.token.onCancellationRequested(() => {
      cancelled = true;
    });

    if (!WS) {
      lines.push("DRY RUN (no WebSocket in host) — validated plan:");
      for (const s of valid) lines.push(`  • ${s.raw}`);
      lines.push(`\n${valid.length} step(s) OK, ${invalid.length} error(s).`);
      await flush();
      exec.end(invalid.length === 0, Date.now());
      return;
    }

    let ws: any;
    try {
      ws = await connect(WS, simUrl);
    } catch (e) {
      lines.push(`✗ could not connect to sim-manager at ${simUrl}: ${e}`);
      lines.push("Is `cargo run --bin sim-manager` running?");
      await flush();
      exec.end(false, Date.now());
      return;
    }

    const acks: string[] = [];
    ws.onmessage = (ev: any) => {
      try {
        const m = JSON.parse(ev.data);
        if (m.type === "ack") acks.push(m.message || "");
      } catch (_e) {
        /* ignore */
      }
    };

    lines.push(`▶ flying ${valid.length} step(s) on ${simUrl}`);
    await flush();

    // Ensure a sim is running so REPL commands take effect.
    ws.send(JSON.stringify({ cmd: "start" }));
    await sleep(pause);

    let okCount = 0;
    for (const s of valid) {
      if (cancelled) {
        lines.push("■ cancelled");
        break;
      }
      if (s.verb === "wait") {
        lines.push(`  ⏱ wait ${s.args[0]}s`);
        await flush();
        await sleep(s.args[0] * 1000);
        continue;
      }
      const before = acks.length;
      ws.send(JSON.stringify({ cmd: "send_mavlink", text: s.raw }));
      await sleep(pause);
      const reply = acks.slice(before).join("; ") || "(sent)";
      lines.push(`  ✓ ${s.raw}  →  ${reply}`);
      okCount++;
      await flush();
    }

    try {
      ws.close();
    } catch (_e) {
      /* ignore */
    }
    lines.push(`\nDone: ${okCount} step(s) flown.`);
    await flush();
    exec.end(!cancelled && invalid.length === 0, Date.now());
  }
}

function connect(WS: any, url: string): Promise<any> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const ws = new WS(url);
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        try {
          ws.close();
        } catch (_e) {
          /* ignore */
        }
        reject(new Error("timeout"));
      }
    }, 3000);
    ws.onopen = () => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        resolve(ws);
      }
    };
    ws.onerror = (e: any) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(new Error(String(e?.message || "error")));
      }
    };
  });
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// Exposed for testing the parser.
export const __test = { parseMission };
