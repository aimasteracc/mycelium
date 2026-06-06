// Thin bridge from VS Code to the Mycelium engine (RFC-0112).
//
// Resolves a workspace-rooted Mycelium client over the published
// @aimasteracc/mycelium-sdk (which itself locates the prebuilt CLI binary —
// no Rust toolchain). The extension adds no engine logic of its own; it is a
// consumer of the existing CLI/MCP surface (Charter §5.13).
import * as vscode from "vscode";
import { Mycelium } from "@aimasteracc/mycelium-sdk";

/**
 * Resolve the workspace root for the given editor's document, or fall back to
 * the first workspace folder. Passing the editor ensures multi-root workspaces
 * route each command to the correct `.mycelium` index.
 */
export function workspaceRoot(editor?: vscode.TextEditor): string | undefined {
  if (editor) {
    const folder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
    if (folder) {
      return folder.uri.fsPath;
    }
  }
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

/**
 * Build a Mycelium client rooted at the workspace. Honors the
 * `mycelium.binaryPath` setting (otherwise the SDK resolves the binary from its
 * bundled platform package or PATH). Returns undefined when no folder is open.
 * Pass `editor` to prefer that editor's workspace folder in a multi-root setup.
 */
export function getClient(editor?: vscode.TextEditor): Mycelium | undefined {
  const root = workspaceRoot(editor);
  if (!root) {
    return undefined;
  }
  const configured = vscode.workspace.getConfiguration("mycelium").get<string>("binaryPath");
  const bin = configured && configured.trim().length > 0 ? configured : undefined;
  return new Mycelium({ root, bin });
}

/** The identifier under the editor's cursor, or undefined. */
export function wordAtCursor(editor: vscode.TextEditor): string | undefined {
  const range = editor.document.getWordRangeAtPosition(editor.selection.active);
  return range ? editor.document.getText(range) : undefined;
}

/**
 * Resolve the symbol the user means: search by the identifier under the cursor,
 * then auto-pick a single hit or prompt a quick-pick on several. Returns the
 * Mycelium symbol path (e.g. `src/lib.rs>App>render`) or undefined if cancelled
 * or not found.
 */
export async function resolveSymbolAtCursor(
  client: Mycelium,
  editor: vscode.TextEditor,
): Promise<string | undefined> {
  const word = wordAtCursor(editor);
  if (!word) {
    vscode.window.showWarningMessage("Mycelium: place the cursor on a symbol name.");
    return undefined;
  }
  const hits = (await client.searchSymbol(word, { limit: 50 })) as unknown[];
  const paths = hits.filter((h): h is string => typeof h === "string");
  if (paths.length === 0) {
    vscode.window.showInformationMessage(`Mycelium: no indexed symbol named "${word}". Run "Mycelium: Index this workspace" first.`);
    return undefined;
  }
  if (paths.length === 1) {
    return paths[0];
  }
  return vscode.window.showQuickPick(paths, {
    title: `Mycelium: which "${word}"?`,
    placeHolder: "Select the symbol",
  });
}
