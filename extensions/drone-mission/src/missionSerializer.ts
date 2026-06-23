// Serializer for `.mission` notebooks (Phase 7).
//
// A `.mission` file is plain text split into cells by marker lines:
//   `# %%`            → a code (mission command) cell
//   `# %% [markdown]` → a markdown cell
// Text before the first marker is the first code cell. The format round-trips.

import * as vscode from "vscode";

const CODE_MARK = "# %%";
const MD_MARK = "# %% [markdown]";

export class MissionSerializer implements vscode.NotebookSerializer {
  deserializeNotebook(content: Uint8Array): vscode.NotebookData {
    const text = new TextDecoder().decode(content);
    const lines = text.split(/\r?\n/);

    type Raw = { kind: vscode.NotebookCellKind; body: string[] };
    const cells: Raw[] = [];
    let cur: Raw = { kind: vscode.NotebookCellKind.Code, body: [] };

    for (const line of lines) {
      const t = line.trim();
      if (t === MD_MARK) {
        cells.push(cur);
        cur = { kind: vscode.NotebookCellKind.Markup, body: [] };
      } else if (t === CODE_MARK) {
        cells.push(cur);
        cur = { kind: vscode.NotebookCellKind.Code, body: [] };
      } else {
        cur.body.push(line);
      }
    }
    cells.push(cur);

    const data = cells
      .map((c) => ({ kind: c.kind, text: c.body.join("\n").replace(/^\n+|\n+$/g, "") }))
      .filter((c, i) => !(i === 0 && c.text === "")) // drop empty leading cell
      .map((c) =>
        new vscode.NotebookCellData(
          c.kind,
          c.text,
          c.kind === vscode.NotebookCellKind.Markup ? "markdown" : "plaintext"
        )
      );

    if (data.length === 0) {
      data.push(new vscode.NotebookCellData(vscode.NotebookCellKind.Code, "", "plaintext"));
    }
    return new vscode.NotebookData(data);
  }

  serializeNotebook(data: vscode.NotebookData): Uint8Array {
    const parts: string[] = [];
    for (const cell of data.cells) {
      const mark = cell.kind === vscode.NotebookCellKind.Markup ? MD_MARK : CODE_MARK;
      parts.push(mark + "\n" + cell.value.replace(/\s+$/g, ""));
    }
    return new TextEncoder().encode(parts.join("\n\n") + "\n");
  }
}
