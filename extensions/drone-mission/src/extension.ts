// Vyuta Drone Mission Notebooks — entry point (Phase 7).
//
// Registers the `.mission` notebook serializer + a controller that flies mission
// cells on the sim-manager (so the simulation panel's 3D viewport shows them in
// real time), and a command to scaffold a new mission notebook.

import * as vscode from "vscode";
import { MissionSerializer } from "./missionSerializer";
import { MissionController } from "./missionController";

const SAMPLE = `# %% [markdown]
# Survey mission
Run each cell to fly it on the Vyuta simulation (start \`sim-manager\` first).

# %%
arm
takeoff 6
goto 20 0 6
wait 2
goto 20 20 6
wait 2
rtl
land
`;

export function activate(context: vscode.ExtensionContext): void {
  const controller = new MissionController();
  context.subscriptions.push(
    vscode.workspace.registerNotebookSerializer("vyuta-mission", new MissionSerializer()),
    controller,
    vscode.commands.registerCommand("vyuta.mission.new", async () => {
      const data = new MissionSerializer().deserializeNotebook(new TextEncoder().encode(SAMPLE));
      const doc = await vscode.workspace.openNotebookDocument("vyuta-mission", data);
      await vscode.window.showNotebookDocument(doc);
    })
  );
}

export function deactivate(): void {
  /* controller disposed via subscriptions */
}
