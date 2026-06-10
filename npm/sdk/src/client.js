// The Mycelium SDK client (RFC-0111).
//
// A thin, typed wrapper over the `mycelium` CLI: each method assembles an argv
// array, spawns the binary, and returns the parsed JSON (or text for the
// format-less commands). It adds no capabilities of its own — every method maps
// 1:1 onto an existing CLI+MCP pair (Charter §5.13). Commands without a typed
// method are reachable via the low-level `run()` escape hatch.
"use strict";

const { resolveBinary } = require("./resolve-binary.js");
const { runJson, runText, MyceliumError } = require("./run.js");

// Argv-smuggling guard. `execFile` already avoids the shell, so there is no
// shell-injection surface — but a user-supplied positional that begins with `-`
// (e.g. a query or path of `--root`) would be re-parsed by the `mycelium` CLI
// as a *flag*, not a value. We can't use a `--` end-of-options separator here
// because our flags (`--root`, `--format`) follow the positionals, so instead
// we reject leading-`-` values: a Hyphae selector, symbol path, or project path
// never legitimately starts with `-`.
function guardPositional(value, label) {
  if (typeof value !== "string") {
    throw new MyceliumError(`${label} must be a string`, {});
  }
  if (value.startsWith("-")) {
    throw new MyceliumError(
      `${label} must not start with "-" (would be parsed as a CLI flag): ${JSON.stringify(value)}`,
      {},
    );
  }
  return value;
}

class Mycelium {
  /**
   * @param {object} [opts]
   * @param {string} [opts.root="."] project root passed as --root
   * @param {string} [opts.bin] explicit binary path (skips resolution)
   * @param {string} [opts.budget] default RFC-0102 budget for budget-aware methods
   * @param {NodeJS.ProcessEnv} [opts.env=process.env] env for binary resolution
   * @param {{ json?: Function, text?: Function }} [opts.runner] injected runners (tests)
   */
  constructor(opts = {}) {
    this.root = opts.root ?? ".";
    this.budget = opts.budget;
    this._bin = opts.bin ?? resolveBinary({ env: opts.env });
    this._json = opts.runner?.json ?? runJson;
    this._text = opts.runner?.text ?? runText;
  }

  /** Build the standard JSON argv: cmd, positionals, --root, --format json, extras. */
  _jsonArgs(cmd, positionals = [], extraFlags = []) {
    return [cmd, ...positionals, "--root", this.root, "--format", "json", ...extraFlags];
  }

  /** Low-level escape hatch: spawn with exactly `args`, JSON-parse stdout. */
  run(args) {
    return this._json(this._bin, args);
  }

  /** Engine version string (plain text, e.g. "mycelium 0.2.1"). */
  version() {
    return this._text(this._bin, ["version"]);
  }

  /** Index a project directory; returns the CLI's plain-text status report. */
  index(path = this.root) {
    return this._text(this._bin, ["index", guardPositional(path, "path")]);
  }

  /** Execute a Hyphae selector; returns `{ matches, count, total_count }`. */
  query(expr) {
    return this._json(this._bin, this._jsonArgs("query", [guardPositional(expr, "query")]));
  }

  /** Case-insensitive substring search over symbol names. */
  searchSymbol(query, opts = {}) {
    const extra = opts.limit == null ? [] : ["--limit", String(opts.limit)];
    return this._json(this._bin, this._jsonArgs("search-symbol", [guardPositional(query, "query")], extra));
  }

  /** All structural info about a symbol (ancestors/descendants/callers/callees). */
  getSymbolInfo(path) {
    return this._json(this._bin, this._jsonArgs("get-symbol-info", [guardPositional(path, "path")]));
  }

  /** Direct callers of a symbol (incoming edges). */
  getCallers(path, opts = {}) {
    const extra = [];
    if (opts.edgeKind) extra.push("--edge-kind", opts.edgeKind);
    if (opts.includeVirtual) extra.push("--include-virtual");
    const budget = opts.budget ?? this.budget;
    if (budget) extra.push("--budget", budget);
    return this._json(this._bin, this._jsonArgs("get-callers", [guardPositional(path, "path")], extra));
  }

  /** Direct callees of a symbol (outgoing edges). */
  getCallees(path, opts = {}) {
    const extra = [];
    if (opts.edgeKind) extra.push("--edge-kind", opts.edgeKind);
    const budget = opts.budget ?? this.budget;
    if (budget) extra.push("--budget", budget);
    return this._json(this._bin, this._jsonArgs("get-callees", [guardPositional(path, "path")], extra));
  }

  /** Task-focused context bundle (the `mycelium_context` twin). */
  context(task, opts = {}) {
    const extra = [];
    if (opts.maxNodes != null) extra.push("--max-nodes", String(opts.maxNodes));
    if (opts.maxCodeBlocks != null) extra.push("--max-code-blocks", String(opts.maxCodeBlocks));
    const budget = opts.budget ?? this.budget;
    if (budget) extra.push("--budget", budget);
    return this._json(this._bin, this._jsonArgs("context", ["--task", task], extra));
  }

  /** Whether an index is loaded, plus node/edge counts. */
  serverStatus() {
    return this._json(this._bin, this._jsonArgs("server-status"));
  }
}

module.exports = { Mycelium, MyceliumError };
