// Type declarations for @aimasteracc/mycelium-sdk (RFC-0111).
// Hand-written — the SDK ships as plain JS, no build step.

/** Error thrown when the CLI fails, is signalled, or emits unparseable JSON. */
export class MyceliumError extends Error {
  name: "MyceliumError";
  /** Process exit code, or null if killed by a signal. */
  code: number | null;
  /** Signal name if the process was killed, else null. */
  signal: string | null;
  /** Captured stderr. */
  stderr: string;
  /** Captured stdout (present when the failure was a JSON parse error). */
  stdout: string;
  /** The argv passed to the binary. */
  args: string[];
}

/** RFC-0102 per-call output budget. */
export type Budget = "auto" | "small" | "medium" | "large" | "disabled" | (string & {});

/** Edge kind for call/dependency traversal. */
export type EdgeKind = "calls" | "imports" | "extends" | "implements" | (string & {});

export interface MyceliumOptions {
  /** Project root, passed as `--root`. Defaults to `"."`. */
  root?: string;
  /** Explicit binary path; skips resolution. Otherwise resolved via
   *  `MYCELIUM_BIN` → platform package → `PATH`. */
  bin?: string;
  /** Default budget applied to budget-aware methods when they omit their own. */
  budget?: Budget;
  /** Environment used for binary resolution. Defaults to `process.env`. */
  env?: NodeJS.ProcessEnv;
}

export interface SearchOptions {
  /** Maximum number of results. */
  limit?: number;
}

export interface CallersOptions {
  edgeKind?: EdgeKind;
  /** Also include callers reaching this symbol via virtual dispatch. */
  includeVirtual?: boolean;
  budget?: Budget;
}

export interface CalleesOptions {
  edgeKind?: EdgeKind;
  budget?: Budget;
}

export interface ContextOptions {
  /** Maximum graph nodes to return (default 30, max 100). */
  maxNodes?: number;
  /** Maximum source snippets to return (default 6, max 25). */
  maxCodeBlocks?: number;
}

/**
 * A thin, typed client over the `mycelium` CLI. Every method maps 1:1 onto an
 * existing CLI+MCP command; commands without a typed method are reachable via
 * {@link Mycelium.run}.
 */
export class Mycelium {
  constructor(opts?: MyceliumOptions);

  /** Project root passed as `--root`. */
  readonly root: string;
  /** Default budget for budget-aware methods. */
  readonly budget?: Budget;

  /** Low-level escape hatch: spawn with exactly `args` and JSON-parse stdout. */
  run(args: string[]): Promise<unknown>;

  /** Engine version string, e.g. `"mycelium 0.2.1"`. */
  version(): Promise<string>;

  /** Index a project directory; resolves to the CLI's plain-text status report. */
  index(path?: string): Promise<string>;

  /** Execute a Hyphae selector; resolves to the parsed JSON result. */
  query(expr: string): Promise<unknown>;

  /** Case-insensitive substring search over symbol names. */
  searchSymbol(query: string, opts?: SearchOptions): Promise<unknown>;

  /** All structural info about a symbol in one call. */
  getSymbolInfo(path: string): Promise<unknown>;

  /** Direct callers of a symbol (incoming edges). */
  getCallers(path: string, opts?: CallersOptions): Promise<unknown>;

  /** Direct callees of a symbol (outgoing edges). */
  getCallees(path: string, opts?: CalleesOptions): Promise<unknown>;

  /** Task-focused context bundle (the `mycelium_context` twin). */
  context(task: string, opts?: ContextOptions): Promise<unknown>;

  /** Whether an index is loaded, plus node/edge counts. */
  serverStatus(): Promise<unknown>;
}

/** Resolve the `mycelium` binary path (or a PATH-resolvable command name). */
export function resolveBinary(opts?: {
  platform?: string;
  arch?: string;
  env?: NodeJS.ProcessEnv;
  resolver?: (request: string) => string;
}): string;
