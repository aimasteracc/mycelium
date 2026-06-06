// Mycelium VS Code extension — Phase 1 MVP (RFC-0112).
//
// A thin client: every command maps onto an existing Mycelium CLI/MCP capability
// surfaced through the SDK. The headline is "Copy context for AI" — one-click,
// token-dense context for the cursor, ready to paste into any AI assistant.
import * as vscode from "vscode";
import { MyceliumError } from "@aimasteracc/mycelium-sdk";
import { getClient, resolveSymbolAtCursor } from "./engine";

/** Run an action with a live client + active editor, mapping errors to toasts. */
async function withClientAndEditor(
  action: (client: NonNullable<ReturnType<typeof getClient>>, editor: vscode.TextEditor) => Promise<void>,
): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage("Mycelium: open a file first.");
    return;
  }
  const client = getClient();
  if (!client) {
    vscode.window.showWarningMessage("Mycelium: open a workspace folder first.");
    return;
  }
  try {
    await action(client, editor);
  } catch (err) {
    const msg = err instanceof MyceliumError ? err.message : String(err);
    vscode.window.showErrorMessage(`Mycelium: ${msg}`);
  }
}

/** The headline feature: copy token-dense context for the cursor to the clipboard. */
async function copyContextForAI(): Promise<void> {
  await withClientAndEditor(async (client, editor) => {
    const selection = editor.document.getText(editor.selection).trim();
    const task =
      selection.length > 0
        ? selection
        : await vscode.window.showInputBox({
            title: "Mycelium: context for AI",
            prompt: "Describe the task (a Hyphae selector or natural-language ask)",
            placeHolder: 'e.g. "trace how login reaches the database"',
          });
    if (!task) {
      return;
    }
    await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Mycelium: gathering context…" },
      async () => {
        const ctx = await client.context(task, { maxNodes: 30 });
        const payload = `<!-- Mycelium context for: ${task} -->\n\`\`\`json\n${JSON.stringify(ctx, null, 2)}\n\`\`\`\n`;
        await vscode.env.clipboard.writeText(payload);
        vscode.window.showInformationMessage("Mycelium: context copied to clipboard — paste it to your AI assistant.");
      },
    );
  });
}

/** Show a symbol's callers or callees in a quick-pick. */
async function showEdges(kind: "callers" | "callees"): Promise<void> {
  await withClientAndEditor(async (client, editor) => {
    const path = await resolveSymbolAtCursor(client, editor);
    if (!path) {
      return;
    }
    const result = (kind === "callers"
      ? await client.getCallers(path)
      : await client.getCallees(path)) as Record<string, unknown>;
    const key = kind === "callers" ? "caller_paths" : "callee_paths";
    const edges = Array.isArray(result[key]) ? (result[key] as unknown[]).filter((e): e is string => typeof e === "string") : [];
    if (edges.length === 0) {
      vscode.window.showInformationMessage(`Mycelium: no ${kind} of ${path}.`);
      return;
    }
    await vscode.window.showQuickPick(edges, {
      title: `Mycelium: ${kind} of ${path} (${edges.length})`,
      placeHolder: `${edges.length} ${kind}`,
    });
  });
}

/** Show full structural info for the symbol at the cursor in an output channel. */
async function showSymbolInfo(output: vscode.OutputChannel): Promise<void> {
  await withClientAndEditor(async (client, editor) => {
    const path = await resolveSymbolAtCursor(client, editor);
    if (!path) {
      return;
    }
    const info = await client.getSymbolInfo(path);
    output.clear();
    output.appendLine(`# Mycelium — ${path}`);
    output.appendLine(JSON.stringify(info, null, 2));
    output.show(true);
  });
}

/** Index (or re-index) the workspace. */
async function indexWorkspace(): Promise<void> {
  await withClientAndEditor(async (client) => {
    await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Mycelium: indexing workspace…" },
      async () => {
        const report = await client.index();
        vscode.window.showInformationMessage(`Mycelium: ${report.split("\n").filter(Boolean).pop() ?? "indexed"}`);
      },
    );
  });
}

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("Mycelium");
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("mycelium.copyContextForAI", copyContextForAI),
    vscode.commands.registerCommand("mycelium.findCallers", () => showEdges("callers")),
    vscode.commands.registerCommand("mycelium.findCallees", () => showEdges("callees")),
    vscode.commands.registerCommand("mycelium.symbolInfo", () => showSymbolInfo(output)),
    vscode.commands.registerCommand("mycelium.index", indexWorkspace),
  );

  if (vscode.workspace.getConfiguration("mycelium").get<boolean>("indexOnActivate")) {
    void indexWorkspace();
  }
}

export function deactivate(): void {
  // nothing to clean up — the SDK spawns short-lived CLI processes per call.
}
