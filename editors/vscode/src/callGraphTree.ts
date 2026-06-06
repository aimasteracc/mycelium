// Call-graph sidebar tree (RFC-0112 Phase 1).
//
// Shows the callers and callees of a focused symbol as an expandable tree.
// Populated on demand from the cursor (`mycelium.showCallGraph`); each leaf
// reveals the symbol's source location on click. Pure consumer of the SDK.
import * as vscode from "vscode";
import { getClient } from "./engine";

type NodeKind = "section" | "symbol" | "info";

class CallNode extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly kind: NodeKind,
    /** Mycelium symbol path for a `symbol` node, else undefined. */
    public readonly symbolPath: string | undefined,
    collapsible: vscode.TreeItemCollapsibleState,
  ) {
    super(label, collapsible);
    this.contextValue = kind;
    if (kind === "symbol" && symbolPath) {
      this.tooltip = symbolPath;
      this.iconPath = new vscode.ThemeIcon("symbol-method");
      this.command = {
        command: "mycelium.revealSymbol",
        title: "Reveal",
        arguments: [symbolPath],
      };
    } else if (kind === "section") {
      this.iconPath = new vscode.ThemeIcon("references");
    }
  }
}

/** Tree of `Callers (n)` and `Callees (m)` sections for the focused symbol. */
export class CallGraphTreeProvider implements vscode.TreeDataProvider<CallNode> {
  private readonly emitter = new vscode.EventEmitter<CallNode | undefined>();
  readonly onDidChangeTreeData = this.emitter.event;

  private focus: string | undefined;
  private callers: string[] = [];
  private callees: string[] = [];

  /** Set the focused symbol and refresh from the engine. */
  async focusOn(path: string): Promise<void> {
    const client = getClient();
    if (!client) {
      return;
    }
    this.focus = path;
    const callers = (await client.getCallers(path)) as Record<string, unknown>;
    const callees = (await client.getCallees(path)) as Record<string, unknown>;
    this.callers = asStrings(callers.caller_paths);
    this.callees = asStrings(callees.callee_paths);
    this.emitter.fire(undefined);
  }

  getTreeItem(node: CallNode): vscode.TreeItem {
    return node;
  }

  getChildren(node?: CallNode): CallNode[] {
    if (!this.focus) {
      return [new CallNode('Run "Mycelium: Show call graph" on a symbol', "info", undefined, vscode.TreeItemCollapsibleState.None)];
    }
    if (!node) {
      const expanded = vscode.TreeItemCollapsibleState.Expanded;
      return [
        new CallNode(`Focus: ${this.focus}`, "info", undefined, vscode.TreeItemCollapsibleState.None),
        new CallNode(`Callers (${this.callers.length})`, "section", undefined, expanded),
        new CallNode(`Callees (${this.callees.length})`, "section", undefined, expanded),
      ];
    }
    if (node.kind === "section") {
      const edges = node.label.startsWith("Callers") ? this.callers : this.callees;
      return edges.map(
        (p) => new CallNode(p, "symbol", p, vscode.TreeItemCollapsibleState.None),
      );
    }
    return [];
  }
}

function asStrings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((e): e is string => typeof e === "string") : [];
}
