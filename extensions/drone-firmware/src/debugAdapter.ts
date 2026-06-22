// Debug adapter for the `vyuta-probe-rs` debug type.
//
// Rather than hand-rolling a GDB stub, this launches `probe-rs dap-server`
// (which speaks the Debug Adapter Protocol over TCP) and points VS Code at it.
// One server process is spawned per debug session and torn down on dispose.

import * as vscode from "vscode";
import * as net from "net";
import { ChildProcess, spawn } from "child_process";

export class ProbeRsDebugAdapterFactory
  implements vscode.DebugAdapterDescriptorFactory, vscode.Disposable
{
  private readonly processes: ChildProcess[] = [];

  async createDebugAdapterDescriptor(
    _session: vscode.DebugSession
  ): Promise<vscode.DebugAdapterDescriptor> {
    const probeRs = vscode.workspace
      .getConfiguration("vyuta.firmware")
      .get<string>("probeRsPath", "probe-rs");

    const port = await freePort();
    const proc = spawn(probeRs, ["dap-server", "--port", String(port)], {
      stdio: "inherit",
    });
    proc.on("error", (err) => {
      vscode.window.showErrorMessage(
        `Vyuta: failed to start '${probeRs} dap-server': ${err.message}`
      );
    });
    this.processes.push(proc);

    await waitForPort(port, 127, 5000);
    return new vscode.DebugAdapterServer(port, "127.0.0.1");
  }

  dispose(): void {
    for (const proc of this.processes) {
      proc.kill();
    }
    this.processes.length = 0;
  }
}

/** Find an unused localhost TCP port. */
function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.unref();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const addr = srv.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      srv.close(() => resolve(port));
    });
  });
}

/** Resolve once `port` is accepting connections, or reject after `timeoutMs`. */
function waitForPort(port: number, _host: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const sock = net.connect(port, "127.0.0.1");
      sock.once("connect", () => {
        sock.destroy();
        resolve();
      });
      sock.once("error", () => {
        sock.destroy();
        if (Date.now() > deadline) {
          reject(new Error(`probe-rs dap-server did not open port ${port} in time`));
        } else {
          setTimeout(tryOnce, 100);
        }
      });
    };
    tryOnce();
  });
}
