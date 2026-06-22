// PX4 build-task provider.
//
// Surfaces a set of preset PX4 `make` targets (SITL airframes + flight-
// controller boards) as VS Code build tasks, and resolves custom `px4` tasks
// authored in tasks.json.

import * as vscode from "vscode";

export interface Px4TaskDefinition extends vscode.TaskDefinition {
  type: "px4";
  /** PX4 make target, e.g. `px4_sitl` or `px4_fmu-v6x_default`. */
  target: string;
  /** Optional SITL airframe/world appended to the target, e.g. `gz_x500`. */
  airframe?: string;
  /** PX4-Autopilot directory; falls back to the `vyuta.firmware.px4Dir` setting. */
  px4Dir?: string;
}

interface Preset {
  target: string;
  airframe?: string;
  label: string;
}

const PRESETS: Preset[] = [
  { target: "px4_sitl", airframe: "gz_x500", label: "SITL · Gazebo x500 (quad)" },
  { target: "px4_sitl", airframe: "gz_standard_vtol", label: "SITL · Gazebo Standard VTOL" },
  { target: "px4_sitl", airframe: "jmavsim", label: "SITL · jMAVSim" },
  { target: "px4_fmu-v6x_default", label: "Board · Pixhawk 6X (fmu-v6x)" },
  { target: "px4_fmu-v6c_default", label: "Board · Pixhawk 6C (fmu-v6c)" },
  { target: "px4_fmu-v5_default", label: "Board · Pixhawk 4 (fmu-v5)" },
];

export class Px4TaskProvider implements vscode.TaskProvider {
  static readonly TYPE = "px4";

  provideTasks(): vscode.Task[] {
    return PRESETS.map((p) =>
      makeTask({ type: "px4", target: p.target, airframe: p.airframe }, p.label)
    );
  }

  resolveTask(task: vscode.Task): vscode.Task | undefined {
    const def = task.definition as Px4TaskDefinition;
    if (def.target) {
      return makeTask(def, undefined);
    }
    return undefined;
  }
}

function resolveVars(value: string): string {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
  return value.replace(/\$\{workspaceFolder\}/g, folder);
}

function px4Dir(def: Px4TaskDefinition): string {
  const configured =
    def.px4Dir ??
    vscode.workspace.getConfiguration("vyuta.firmware").get<string>("px4Dir") ??
    "${workspaceFolder}";
  return resolveVars(configured);
}

export function makeTask(def: Px4TaskDefinition, label: string | undefined): vscode.Task {
  const command = def.airframe
    ? `make ${def.target} ${def.airframe}`
    : `make ${def.target}`;
  const name = label ?? (def.airframe ? `${def.target} ${def.airframe}` : def.target);

  const task = new vscode.Task(
    def,
    vscode.TaskScope.Workspace,
    name,
    Px4TaskProvider.TYPE,
    new vscode.ShellExecution(command, { cwd: px4Dir(def) })
  );
  task.group = vscode.TaskGroup.Build;
  task.detail = `${command}  (in ${px4Dir(def)})`;
  return task;
}
