// Unit tests for the SDK runner (spawn + parse + error model, RFC-0111).
// Run with: node --test
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { runJson, runText, MyceliumError } = require("../src/run.js");

/** Build a fake spawn that resolves to a canned result and records its call. */
function fakeSpawn(result, calls) {
  return (bin, args) => {
    calls.push({ bin, args });
    return Promise.resolve(result);
  };
}

test("runJson parses stdout JSON on a clean exit", async () => {
  const calls = [];
  const spawn = fakeSpawn({ status: 0, signal: null, stdout: '["a","b"]', stderr: "" }, calls);
  const out = await runJson("mycelium", ["query", "#x", "--format", "json"], { spawn });
  assert.deepEqual(out, ["a", "b"]);
  assert.deepEqual(calls, [{ bin: "mycelium", args: ["query", "#x", "--format", "json"] }]);
});

test("runJson throws MyceliumError carrying code + stderr on non-zero exit", async () => {
  const spawn = fakeSpawn({ status: 2, signal: null, stdout: "", stderr: "boom" }, []);
  await assert.rejects(
    () => runJson("mycelium", ["query", "#x"], { spawn }),
    (err) => {
      assert.ok(err instanceof MyceliumError);
      assert.ok(err instanceof Error);
      assert.equal(err.code, 2);
      assert.equal(err.stderr, "boom");
      assert.deepEqual(err.args, ["query", "#x"]);
      return true;
    },
  );
});

test("runJson throws MyceliumError on unparseable JSON", async () => {
  const spawn = fakeSpawn({ status: 0, signal: null, stdout: "not json", stderr: "" }, []);
  await assert.rejects(
    () => runJson("mycelium", ["query", "#x"], { spawn }),
    (err) => {
      assert.ok(err instanceof MyceliumError);
      assert.match(err.message, /invalid JSON/i);
      return true;
    },
  );
});

test("runJson throws MyceliumError when the process is killed by a signal", async () => {
  const spawn = fakeSpawn({ status: null, signal: "SIGKILL", stdout: "", stderr: "" }, []);
  await assert.rejects(
    () => runJson("mycelium", ["query"], { spawn }),
    (err) => {
      assert.ok(err instanceof MyceliumError);
      assert.equal(err.signal, "SIGKILL");
      return true;
    },
  );
});

test("runText returns the trimmed stdout string", async () => {
  const spawn = fakeSpawn({ status: 0, signal: null, stdout: "mycelium 0.2.1\n", stderr: "" }, []);
  const out = await runText("mycelium", ["version"], { spawn });
  assert.equal(out, "mycelium 0.2.1");
});

test("runText still throws on a non-zero exit", async () => {
  const spawn = fakeSpawn({ status: 1, signal: null, stdout: "", stderr: "nope" }, []);
  await assert.rejects(() => runText("mycelium", ["version"], { spawn }), MyceliumError);
});
