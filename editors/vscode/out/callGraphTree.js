"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.CallGraphTreeProvider = void 0;
// Call-graph sidebar tree (RFC-0112 Phase 1).
//
// Shows the callers and callees of a focused symbol as an expandable tree.
// Populated on demand from the cursor (`mycelium.showCallGraph`); each leaf
// reveals the symbol's source location on click. Pure consumer of the SDK.
const vscode = __importStar(require("vscode"));
const engine_1 = require("./engine");
class CallNode extends vscode.TreeItem {
    label;
    kind;
    symbolPath;
    section;
    constructor(label, kind, 
    /** Mycelium symbol path for a `symbol` node, else undefined. */
    symbolPath, 
    /** For a `section` node: which edge set it lists. */
    section, collapsible) {
        super(label, collapsible);
        this.label = label;
        this.kind = kind;
        this.symbolPath = symbolPath;
        this.section = section;
        this.contextValue = kind;
        if (kind === "symbol" && symbolPath) {
            this.tooltip = symbolPath;
            this.iconPath = new vscode.ThemeIcon("symbol-method");
            this.command = {
                command: "mycelium.revealSymbol",
                title: "Reveal",
                arguments: [symbolPath],
            };
        }
        else if (kind === "section") {
            this.iconPath = new vscode.ThemeIcon("references");
        }
    }
}
/** Tree of `Callers (n)` and `Callees (m)` sections for the focused symbol. */
class CallGraphTreeProvider {
    emitter = new vscode.EventEmitter();
    onDidChangeTreeData = this.emitter.event;
    focus;
    callers = [];
    callees = [];
    /** Set the focused symbol and refresh from the engine. */
    async focusOn(path) {
        const client = (0, engine_1.getClient)();
        if (!client) {
            return;
        }
        // Fetch both first; commit state atomically so a failure can't leave the
        // tree showing a new label over the previous symbol's edges.
        const [callersRaw, calleesRaw] = await Promise.all([client.getCallers(path), client.getCallees(path)]);
        this.focus = path;
        this.callers = asStrings(callersRaw.caller_paths);
        this.callees = asStrings(calleesRaw.callee_paths);
        this.emitter.fire(undefined);
    }
    getTreeItem(node) {
        return node;
    }
    getChildren(node) {
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
exports.CallGraphTreeProvider = CallGraphTreeProvider;
function asStrings(value) {
    return Array.isArray(value) ? value.filter((e) => typeof e === "string") : [];
}
//# sourceMappingURL=callGraphTree.js.map