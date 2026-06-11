// Unit tests for the Mycelium client argv assembly (RFC-0111).
// A fake runner records the argv each method emits, so these tests are fully
// hermetic — no real binary required.
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { Mycelium, MyceliumError } = require("../index.js");

/** A client whose json/text runners just record argv and return a canned value. */
function spyClient(opts = {}) {
  const calls = [];
  const runner = {
    json: (bin, args) => {
      calls.push({ kind: "json", bin, args });
      return Promise.resolve({ ok: true });
    },
    text: (bin, args) => {
      calls.push({ kind: "text", bin, args });
      return Promise.resolve("mycelium 0.2.1");
    },
  };
  const client = new Mycelium({ bin: "mycelium", runner, ...opts });
  return { client, calls };
}

test("public surface is exported", () => {
  assert.equal(typeof Mycelium, "function");
  assert.equal(typeof MyceliumError, "function");
});

test("version() runs `version` as text and trims the result", async () => {
  const { client, calls } = spyClient();
  const v = await client.version();
  assert.equal(v, "mycelium 0.2.1");
  assert.deepEqual(calls, [{ kind: "text", bin: "mycelium", args: ["version"] }]);
});

test("index() runs `index <path>` as text (no --format)", async () => {
  const { client, calls } = spyClient();
  await client.index("./src");
  assert.deepEqual(calls[0], { kind: "text", bin: "mycelium", args: ["index", "./src"] });
});

test("index() defaults the path to the client root", async () => {
  const { client, calls } = spyClient({ root: "/proj" });
  await client.index();
  assert.deepEqual(calls[0].args, ["index", "/proj"]);
});

test("query() appends --root and --format json", async () => {
  const { client, calls } = spyClient();
  await client.query("#login");
  assert.deepEqual(calls[0], {
    kind: "json",
    bin: "mycelium",
    args: ["query", "#login", "--root", ".", "--format", "json"],
  });
});

test("query() honours a custom root from the constructor", async () => {
  const { client, calls } = spyClient({ root: "/repo" });
  await client.query(".function");
  assert.deepEqual(calls[0].args, ["query", ".function", "--root", "/repo", "--format", "json"]);
});

test("searchSymbol() passes the query, --limit, --root and --format json", async () => {
  const { client, calls } = spyClient();
  await client.searchSymbol("login", { limit: 10 });
  assert.deepEqual(calls[0].args, [
    "search-symbol", "login", "--root", ".", "--format", "json", "--limit", "10",
  ]);
});

test("getSymbolInfo() builds the kebab-case subcommand", async () => {
  const { client, calls } = spyClient();
  await client.getSymbolInfo("src/lib.rs>App>render");
  assert.deepEqual(calls[0].args, [
    "get-symbol-info", "src/lib.rs>App>render", "--root", ".", "--format", "json",
  ]);
});

test("getCallers() emits --edge-kind, --include-virtual and --budget", async () => {
  const { client, calls } = spyClient();
  await client.getCallers("a>b", { edgeKind: "calls", includeVirtual: true, budget: "small" });
  assert.deepEqual(calls[0].args, [
    "get-callers", "a>b", "--root", ".", "--format", "json",
    "--edge-kind", "calls", "--include-virtual", "--budget", "small",
  ]);
});

test("getCallers() omits --include-virtual when false and --budget when unset", async () => {
  const { client, calls } = spyClient();
  await client.getCallers("a>b");
  assert.deepEqual(calls[0].args, ["get-callers", "a>b", "--root", ".", "--format", "json"]);
});

test("getCallees() emits --edge-kind and --budget", async () => {
  const { client, calls } = spyClient();
  await client.getCallees("a>b", { edgeKind: "imports", budget: "large" });
  assert.deepEqual(calls[0].args, [
    "get-callees", "a>b", "--root", ".", "--format", "json",
    "--edge-kind", "imports", "--budget", "large",
  ]);
});

test("context() uses --task and optional --max-nodes / --max-code-blocks", async () => {
  const { client, calls } = spyClient();
  await client.context("trace ServeHTTP to HandlerFunc", { maxNodes: 30, maxCodeBlocks: 6 });
  assert.deepEqual(calls[0].args, [
    "context", "--task", "trace ServeHTTP to HandlerFunc", "--root", ".", "--format", "json",
    "--max-nodes", "30", "--max-code-blocks", "6",
  ]);
});

test("context() forwards an explicit --budget", async () => {
  const { client, calls } = spyClient();
  await client.context("trace X to Y", { budget: "disabled" });
  assert.deepEqual(calls[0].args, [
    "context", "--task", "trace X to Y", "--root", ".", "--format", "json", "--budget", "disabled",
  ]);
});

test("context() falls back to the constructor-level budget", async () => {
  const { client, calls } = spyClient({ budget: "small" });
  await client.context("trace X to Y");
  assert.deepEqual(calls[0].args, [
    "context", "--task", "trace X to Y", "--root", ".", "--format", "json", "--budget", "small",
  ]);
});

test("serverStatus() builds the kebab-case subcommand with --format json", async () => {
  const { client, calls } = spyClient();
  await client.serverStatus();
  assert.deepEqual(calls[0].args, ["server-status", "--root", ".", "--format", "json"]);
});

test("run() is a raw escape hatch — passes argv through untouched", async () => {
  const { client, calls } = spyClient();
  await client.run(["get-dead-symbols", "--prefix", "src/", "--format", "json"]);
  assert.deepEqual(calls[0], {
    kind: "json",
    bin: "mycelium",
    args: ["get-dead-symbols", "--prefix", "src/", "--format", "json"],
  });
});

test("a constructor-level budget is applied when a method omits its own", async () => {
  const { client, calls } = spyClient({ budget: "small" });
  await client.getCallees("a>b");
  assert.deepEqual(calls[0].args, [
    "get-callees", "a>b", "--root", ".", "--format", "json", "--budget", "small",
  ]);
});
