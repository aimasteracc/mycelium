// End-to-end integration test against a real `mycelium` binary (RFC-0111).
//
// Skipped unless a binary is available — set MYCELIUM_BIN to its path (e.g.
// `MYCELIUM_BIN=target/debug/mycelium node --test`). This guards CI runs that
// have no built binary while still proving the JSON contract locally / in the
// release job.
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const os = require("node:os");
const fs = require("node:fs");

const { Mycelium, MyceliumError } = require("../index.js");

const BIN = process.env.MYCELIUM_BIN;
const skip = BIN ? false : "set MYCELIUM_BIN to run the live integration test";

test("indexes a tiny project and round-trips JSON through the SDK", { skip }, async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "mycelium-sdk-"));
  try {
    fs.writeFileSync(
      path.join(dir, "main.py"),
      "def helper():\n    return 1\n\ndef main():\n    return helper()\n",
    );
    const m = new Mycelium({ root: dir, bin: BIN });

    const version = await m.version();
    assert.match(version, /^mycelium \d+\.\d+\.\d+/);

    await m.index();

    const status = await m.serverStatus();
    assert.ok(status.node_count > 0, "expected indexed nodes");

    const functions = await m.query(".function");
    assert.ok(Array.isArray(functions), "query returns a JSON array");
    assert.ok(functions.length >= 2, "expected at least helper + main");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("surfaces CLI failures as MyceliumError", { skip }, async () => {
  const m = new Mycelium({ root: ".", bin: BIN });
  await assert.rejects(() => m.query("((("), MyceliumError);
});
