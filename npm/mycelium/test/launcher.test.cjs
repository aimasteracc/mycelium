// Unit tests for the CLI launcher's platform resolution (RFC-0110).
// Run with: node --test  (Node 18+ has the built-in test runner).
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { platformPackage, binaryName, resolveBinary, PLATFORMS, SCOPE } = require("../bin/mycelium.cjs");

test("platformPackage maps every supported platform under the scope", () => {
  assert.equal(platformPackage("darwin", "arm64"), `${SCOPE}/mycelium-darwin-arm64`);
  assert.equal(platformPackage("darwin", "x64"), `${SCOPE}/mycelium-darwin-x64`);
  assert.equal(platformPackage("linux", "x64"), `${SCOPE}/mycelium-linux-x64-gnu`);
  assert.equal(platformPackage("linux", "arm64"), `${SCOPE}/mycelium-linux-arm64-gnu`);
  assert.equal(platformPackage("win32", "x64"), `${SCOPE}/mycelium-win32-x64`);
});

test("platformPackage returns null for unsupported platform/arch", () => {
  assert.equal(platformPackage("sunos", "sparc"), null);
  assert.equal(platformPackage("linux", "ia32"), null);
});

test("binaryName appends .exe only on Windows", () => {
  assert.equal(binaryName("win32"), "mycelium.exe");
  assert.equal(binaryName("darwin"), "mycelium");
  assert.equal(binaryName("linux"), "mycelium");
});

test("resolveBinary asks the resolver for the platform package's binary", () => {
  const seen = [];
  const fakeResolver = (req) => {
    seen.push(req);
    return `/fake/node_modules/${req}`;
  };
  const got = resolveBinary("darwin", "arm64", fakeResolver);
  assert.equal(got, `/fake/node_modules/${SCOPE}/mycelium-darwin-arm64/bin/mycelium`);
  assert.deepEqual(seen, [`${SCOPE}/mycelium-darwin-arm64/bin/mycelium`]);
});

test("resolveBinary requests the .exe on Windows", () => {
  const got = resolveBinary("win32", "x64", (req) => `/nm/${req}`);
  assert.equal(got, `/nm/${SCOPE}/mycelium-win32-x64/bin/mycelium.exe`);
});

test("resolveBinary returns null when the package is not installed (resolver throws)", () => {
  const throwing = () => {
    throw new Error("Cannot find module");
  };
  assert.equal(resolveBinary("linux", "x64", throwing), null);
});

test("resolveBinary returns null for an unsupported platform without calling the resolver", () => {
  let called = false;
  const got = resolveBinary("sunos", "sparc", () => {
    called = true;
    return "x";
  });
  assert.equal(got, null);
  assert.equal(called, false);
});

test("every PLATFORMS entry has a non-empty suffix", () => {
  for (const [key, suffix] of Object.entries(PLATFORMS)) {
    assert.ok(suffix && suffix.startsWith("mycelium-"), `bad suffix for ${key}: ${suffix}`);
  }
});
