// Mycelium VS Code extension — Phase 1 MVP (RFC-0112).
//
// A thin client: every command maps onto an existing Mycelium CLI/MCP capability
// surfaced through the SDK. The headline is "Copy context for AI" — one-click,
// token-dense, AI-shaped context for the cursor, ready to paste into any
// assistant.
import * as vscode from "vscode";
import * as nodePath from "path";
import { MyceliumError } from "@aimasteracc/mycelium-sdk";
import { getClient, resolveSymbolAtCursor, workspaceRoot } from "./engine";
import { CallGraphTreeProvider } from "./callGraphTree";

type Client = NonNullable<ReturnType<typeof getClient>>;

function errMessage(err: unknown): string {
  return err instanceof MyceliumError ? err.message : String(err);
}

/** Run an action with a live client (no editor required); map errors to toasts. */
async function withClient(action: (client: Client) => Promise<void>): Promise<void> {
  const client = getClient();
  if (!client) {
    vscode.window.showWarningMessage("Mycelium: open a workspace folder first.");
    return;
  }
  try {
    await action(client);
  } catch (err) {
    vscode.window.showErrorMessage(`Mycelium: ${errMessage(err)}`);
  }
}

/** Run an action that needs both a client and the active editor. */
async function withClientAndEditor(
  action: (client: Client, editor: vscode.TextEditor) => Promise<void>,
): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage("Mycelium: open a file first.");
    return;
  }
  await withClient((client) => action(client, editor));
}

/** Index the workspace, returning the CLI's last status line. */
async function runIndex(client: Client): Promise<string> {
  return vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: "Mycelium: indexing workspace…" },
    async () => {
      const report = await client.index();
      return report.split("\n").filter(Boolean).pop() ?? "indexed";
    },
  );
}

function looksLikeMissingIndex(err: unknown): boolean {
  const msg = errMessage(err).toLowerCase();
  return msg.includes("index") || msg.includes("not found") || msg.includes("no such");
}

/**
 * Run `attempt`; if it fails like a missing index, offer to index and retry
 * once — turning the first-run dead-end into a one-click success.
 */
async function withFirstRunIndex<T>(client: Client, attempt: () => Promise<T>): Promise<T | undefined> {
  try {
    return await attempt();
  } catch (err) {
    if (!looksLikeMissingIndex(err)) {
      throw err;
    }
    const choice = await vscode.window.showInformationMessage(
      "Mycelium hasn't indexed this workspace yet.",
      { modal: true },
      "Index now",
    );
    if (choice !== "Index now") {
      return undefined;
    }
    await runIndex(client);
    return attempt();
  }
}

/** The headline feature: copy AI-shaped, token-dense context for the cursor. */
async function copyContextForAI(): Promise<void> {
  await withClientAndEditor(async (client, editor) => {
    const selection = editor.document.getText(editor.selection).trim();
    let task: string | undefined;
    if (selection.length > 0 && selection.length <= 400) {
      task = selection;
    } else {
      // No (small) selection: default to the symbol under the cursor.
      task = await resolveSymbolAtCursor(client, editor);
      if (!task) {
        task = await vscode.window.showInputBox({
          title: "Mycelium: context for AI",
          prompt: "Describe the task (a Hyphae selector or natural-language ask)",
          placeHolder: 'e.g. "trace how login reaches the database"',
        });
      }
    }
    if (!task) {
      return;
    }
    const resolvedTask = task;
    const ctx = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: "Mycelium: gathering context…" },
      () => withFirstRunIndex(client, () => client.context(resolvedTask, { maxNodes: 30 })),
    );
    if (ctx === undefined) {
      return;
    }
    await vscode.env.clipboard.writeText(formatContextForAI(resolvedTask, ctx));
    vscode.window.showInformationMessage(
      "Mycelium: context copied — paste it to your AI assistant. (May include source from your workspace.)",
    );
  });
}

/** Shape the context as a ready-to-send prompt, not a raw data dump. */
function formatContextForAI(task: string, ctx: unknown): string {
  return [
    `Here is the relevant code context for: "${task}".`,
    "It is a slice of the project's code-intelligence graph (Mycelium). Use it to ground your answer.",
    "",
    "```json",
    JSON.stringify(ctx), // compact — pretty-printing only burns tokens
    "```",
    "",
    "## Your task:",
    "",
  ].join("\n");
}

/** Show a symbol's callers or callees; selecting one navigates to it. */
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
    const edges = asStrings(result[key]);
    if (edges.length === 0) {
      vscode.window.showInformationMessage(`Mycelium: no ${kind} of ${path}.`);
      return;
    }
    const picked = await vscode.window.showQuickPick(edges, {
      title: `Mycelium: ${kind} of ${path} (${edges.length})`,
      placeHolder: `${edges.length} ${kind} — select to open`,
    });
    if (picked) {
      await revealSymbol(picked);
    }
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

/** Populate the call-graph sidebar from the symbol at the cursor. */
async function showCallGraph(tree: CallGraphTreeProvider): Promise<void> {
  await withClientAndEditor(async (client, editor) => {
    const path = await resolveSymbolAtCursor(client, editor);
    if (path) {
      await tree.focusOn(path);
      // VS Code auto-synthesizes `<viewId>.focus`; ignore if the view is hidden.
      await vscode.commands.executeCommand("mycelium.callGraph.focus").then(undefined, () => undefined);
    }
  });
}

/** Open the file for a Mycelium symbol path and reveal its line (best-effort). */
async function revealSymbol(symbolPath: string | undefined): Promise<void> {
  if (!symbolPath || symbolPath.startsWith("-")) {
    return; // guard undefined (palette) + argv-smuggling (leading dash)
  }
  const root = workspaceRoot();
  const client = getClient();
  if (!root || !client) {
    return;
  }
  const file = symbolPath.split(">")[0];
  const resolved = nodePath.resolve(root, file);
  const rootResolved = nodePath.resolve(root);
  if (resolved !== rootResolved && !resolved.startsWith(rootResolved + nodePath.sep)) {
    vscode.window.showWarningMessage(`Mycelium: "${symbolPath}" resolves outside the workspace — blocked.`);
    return;
  }
  try {
    const editor = await vscode.window.showTextDocument(
      await vscode.workspace.openTextDocument(vscode.Uri.file(resolved)),
    );
    // `get-source-span` has no typed SDK method — use the raw escape hatch.
    const span = (await client.run([
      "get-source-span",
      symbolPath,
      "--root",
      root,
      "--format",
      "json",
    ])) as Record<string, unknown>;
    const lineRaw = (span.start_line ?? span.line) as unknown;
    if (typeof lineRaw === "number" && lineRaw > 0) {
      const pos = new vscode.Position(lineRaw - 1, 0);
      editor.selection = new vscode.Selection(pos, pos);
      editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
    }
  } catch (err) {
    vscode.window.showWarningMessage(`Mycelium: could not reveal ${symbolPath}: ${errMessage(err)}`);
  }
}

function asStrings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((e): e is string => typeof e === "string") : [];
}

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("Mycelium");
  const callGraph = new CallGraphTreeProvider();
  context.subscriptions.push(
    output,
    vscode.window.registerTreeDataProvider("mycelium.callGraph", callGraph),
    vscode.commands.registerCommand("mycelium.copyContextForAI", copyContextForAI),
    vscode.commands.registerCommand("mycelium.findCallers", () => showEdges("callers")),
    vscode.commands.registerCommand("mycelium.findCallees", () => showEdges("callees")),
    vscode.commands.registerCommand("mycelium.symbolInfo", () => showSymbolInfo(output)),
    vscode.commands.registerCommand("mycelium.index", () =>
      withClient(async (client) => {
        vscode.window.showInformationMessage(`Mycelium: ${await runIndex(client)}`);
      }),
    ),
    vscode.commands.registerCommand("mycelium.showCallGraph", () => showCallGraph(callGraph)),
    vscode.commands.registerCommand("mycelium.revealSymbol", (p: string) => revealSymbol(p)),
  );

  if (vscode.workspace.getConfiguration("mycelium").get<boolean>("indexOnActivate")) {
    void withClient(async (client) => {
      vscode.window.showInformationMessage(`Mycelium: ${await runIndex(client)}`);
    });
  }
}

export function deactivate(): void {
  // nothing to clean up — the SDK spawns short-lived CLI processes per call.
}
