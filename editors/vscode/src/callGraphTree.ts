// Call-graph sidebar tree (RFC-0112 Phase 1).
//
// Shows the callers and callees of a focused symbol as an expandable tree.
// Populated on demand from the cursor (`mycelium.showCallGraph`); each leaf
// reveals the symbol's source location on click. Pure consumer of the SDK.
import * as vscode from "vscode";
import { getClient } from "./engine";

type NodeKind = "section" | "symbol" | "info";
type Section = "callers" | "callees";

class CallNode extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly kind: NodeKind,
    /** Mycelium symbol path for a `symbol` node, else undefined. */
    public readonly symbolPath: string | undefined,
    /** For a `section` node: which edge set it lists. */
    public readonly section: Section | undefined,
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
    // Fetch both first; commit state atomically so a failure can't leave the
    // tree showing a new label over the previous symbol's edges.
    const [callersRaw, calleesRaw] = await Promise.all([client.getCallers(path), client.getCallees(path)]);
    this.focus = path;
    this.callers = asStrings((callersRaw as Record<string, unknown>).caller_paths);
    this.callees = asStrings((calleesRaw as Record<string, unknown>).callee_paths);
    this.emitter.fire(undefined);
  }

  getTreeItem(node: CallNode): vscode.TreeItem {
    return node;
  }

  getChildren(node?: CallNode): CallNode[] {
    const none = vscode.TreeItemCollapsibleState.None;
    if (!this.focus) {
      return [new CallNode('Run "Mycelium: Show call graph" on a symbol', "info", undefined, undefined, none)];
    }
    if (!node) {
      const expanded = vscode.TreeItemCollapsibleState.Expanded;
      return [
        new CallNode(`Focus: ${this.focus}`, "info", undefined, undefined, none),
        new CallNode(`Callers (${this.callers.length})`, "section", undefined, "callers", expanded),
        new CallNode(`Callees (${this.callees.length})`, "section", undefined, "callees", expanded),
      ];
    }
    if (node.kind === "section" && node.section) {
      const edges = node.section === "callers" ? this.callers : this.callees;
      return edges.map((p) => new CallNode(p, "symbol", p, undefined, none));
    }
    return [];
  }
}

function asStrings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((e): e is string => typeof e === "string") : [];
}
