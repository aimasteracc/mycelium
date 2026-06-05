// Process runner for the Mycelium SDK (RFC-0111).
//
// Spawns the resolved `mycelium` binary with an argv array (never a shell
// string — no injection surface), captures stdout/stderr, and maps the result
// to a parsed value or a typed error. The spawn function is injectable so the
// runner is unit-testable with no real binary.
"use strict";

const { execFile } = require("node:child_process");

/** Error thrown when the CLI fails, is signalled, or emits unparseable JSON. */
class MyceliumError extends Error {
  /**
   * @param {string} message
   * @param {{ code?: number|null, signal?: string|null, stderr?: string, stdout?: string, args?: string[] }} [info]
   */
  constructor(message, info = {}) {
    super(message);
    this.name = "MyceliumError";
    this.code = info.code ?? null;
    this.signal = info.signal ?? null;
    this.stderr = info.stderr ?? "";
    this.stdout = info.stdout ?? "";
    this.args = info.args ?? [];
  }
}

/**
 * Default spawn: run `bin args`, resolve `{ status, signal, stdout, stderr }`.
 * Never rejects — process-level failures are surfaced as a non-zero status so
 * the caller's error model is the single source of truth.
 *
 * @param {string} bin
 * @param {string[]} args
 * @returns {Promise<{ status: number|null, signal: string|null, stdout: string, stderr: string }>}
 */
function defaultSpawn(bin, args) {
  return new Promise((resolve) => {
    execFile(bin, args, { maxBuffer: 64 * 1024 * 1024 }, (error, stdout, stderr) => {
      if (error && typeof error.code !== "number" && !error.signal) {
        // Spawn failure (e.g. ENOENT): no exit code — model as status 127.
        resolve({ status: 127, signal: null, stdout: "", stderr: String(error.message) });
        return;
      }
      resolve({
        status: error ? (typeof error.code === "number" ? error.code : 1) : 0,
        signal: error?.signal ?? null,
        stdout: stdout ?? "",
        stderr: stderr ?? "",
      });
    });
  });
}

/** Shared guard: throw on signal / non-zero exit. */
async function runRaw(bin, args, { spawn = defaultSpawn } = {}) {
  const { status, signal, stdout, stderr } = await spawn(bin, args);
  if (signal) {
    throw new MyceliumError(`mycelium was killed by signal ${signal}`, { signal, stderr, args });
  }
  if (status !== 0) {
    throw new MyceliumError(
      `mycelium exited with code ${status}${stderr ? `: ${stderr.trim()}` : ""}`,
      { code: status, stderr, stdout, args },
    );
  }
  return stdout;
}

/** Run the CLI and JSON-parse its stdout. Throws MyceliumError on any failure. */
async function runJson(bin, args, opts = {}) {
  const stdout = await runRaw(bin, args, opts);
  try {
    return JSON.parse(stdout);
  } catch (err) {
    throw new MyceliumError(`mycelium produced invalid JSON: ${err.message}`, {
      code: 0,
      stdout,
      args,
    });
  }
}

/** Run the CLI and return its trimmed stdout text. Throws on any failure. */
async function runText(bin, args, opts = {}) {
  const stdout = await runRaw(bin, args, opts);
  return stdout.trim();
}

module.exports = { MyceliumError, defaultSpawn, runJson, runText };
