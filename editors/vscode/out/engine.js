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
exports.workspaceRoot = workspaceRoot;
exports.getClient = getClient;
exports.wordAtCursor = wordAtCursor;
exports.resolveSymbolAtCursor = resolveSymbolAtCursor;
// Thin bridge from VS Code to the Mycelium engine (RFC-0112).
//
// Resolves a workspace-rooted Mycelium client over the published
// @aimasteracc/mycelium-sdk (which itself locates the prebuilt CLI binary —
// no Rust toolchain). The extension adds no engine logic of its own; it is a
// consumer of the existing CLI/MCP surface (Charter §5.13).
const vscode = __importStar(require("vscode"));
const mycelium_sdk_1 = require("@aimasteracc/mycelium-sdk");
/** The first workspace folder's filesystem path, or undefined if none is open. */
function workspaceRoot() {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
/**
 * Build a Mycelium client rooted at the workspace. Honors the
 * `mycelium.binaryPath` setting (otherwise the SDK resolves the binary from its
 * bundled platform package or PATH). Returns undefined when no folder is open.
 */
function getClient() {
    const root = workspaceRoot();
    if (!root) {
        return undefined;
    }
    const configured = vscode.workspace.getConfiguration("mycelium").get("binaryPath");
    const bin = configured && configured.trim().length > 0 ? configured : undefined;
    return new mycelium_sdk_1.Mycelium({ root, bin });
}
/** The identifier under the editor's cursor, or undefined. */
function wordAtCursor(editor) {
    const range = editor.document.getWordRangeAtPosition(editor.selection.active);
    return range ? editor.document.getText(range) : undefined;
}
/**
 * Resolve the symbol the user means: search by the identifier under the cursor,
 * then auto-pick a single hit or prompt a quick-pick on several. Returns the
 * Mycelium symbol path (e.g. `src/lib.rs>App>render`) or undefined if cancelled
 * or not found.
 */
async function resolveSymbolAtCursor(client, editor) {
    const word = wordAtCursor(editor);
    if (!word) {
        vscode.window.showWarningMessage("Mycelium: place the cursor on a symbol name.");
        return undefined;
    }
    let paths = await search(client, word);
    if (paths.length === 0) {
        // Likely an un-indexed workspace on first run: offer to index, then retry once.
        const choice = await vscode.window.showInformationMessage(`Mycelium: no indexed symbol named "${word}".`, { modal: true }, "Index now");
        if (choice !== "Index now") {
            return undefined;
        }
        await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: "Mycelium: indexing workspace…" }, () => client.index());
        paths = await search(client, word);
        if (paths.length === 0) {
            vscode.window.showInformationMessage(`Mycelium: still no symbol named "${word}" after indexing.`);
            return undefined;
        }
    }
    if (paths.length === 1) {
        return paths[0];
    }
    // Bias toward symbols defined in the active file, so a common name is unambiguous.
    const here = vscode.workspace.asRelativePath(editor.document.uri);
    paths.sort((a, b) => Number(b.startsWith(here)) - Number(a.startsWith(here)));
    return vscode.window.showQuickPick(paths, {
        title: `Mycelium: which "${word}"? (this file first)`,
        placeHolder: "Select the symbol",
    });
}
async function search(client, word) {
    const hits = (await client.searchSymbol(word, { limit: 200 }));
    return Array.isArray(hits) ? hits.filter((h) => typeof h === "string") : [];
}
//# sourceMappingURL=engine.js.map