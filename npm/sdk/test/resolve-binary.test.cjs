// Unit tests for SDK binary resolution (RFC-0111).
// Run with: node --test
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const {
  resolveBinary,
  platformPackage,
  binaryName,
  SCOPE,
} = require("../src/resolve-binary.js");

test("MYCELIUM_BIN env var overrides everything", () => {
  const got = resolveBinary({
    platform: "darwin",
    arch: "arm64",
    env: { MYCELIUM_BIN: "/custom/mycelium" },
    resolver: () => {
      throw new Error("resolver must not be consulted when env is set");
    },
  });
  assert.equal(got, "/custom/mycelium");
});

test("resolves the per-platform package binary via the injected resolver", () => {
  const seen = [];
  const got = resolveBinary({
    platform: "darwin",
    arch: "arm64",
    env: {},
    resolver: (req) => {
      seen.push(req);
      return `/nm/${req}`;
    },
  });
  assert.equal(got, `/nm/${SCOPE}/mycelium-darwin-arm64/bin/mycelium`);
  assert.deepEqual(seen, [`${SCOPE}/mycelium-darwin-arm64/bin/mycelium`]);
});

test("requests the .exe binary on Windows", () => {
  const got = resolveBinary({
    platform: "win32",
    arch: "x64",
    env: {},
    resolver: (req) => `/nm/${req}`,
  });
  assert.equal(got, `/nm/${SCOPE}/mycelium-win32-x64/bin/mycelium.exe`);
});

test("falls back to the PATH command name when the package is not installed", () => {
  const got = resolveBinary({
    platform: "linux",
    arch: "x64",
    env: {},
    resolver: () => {
      throw new Error("Cannot find module");
    },
  });
  assert.equal(got, "mycelium");
});

test("falls back to mycelium.exe on Windows when nothing is installed", () => {
  const got = resolveBinary({
    platform: "win32",
    arch: "x64",
    env: {},
    resolver: () => {
      throw new Error("Cannot find module");
    },
  });
  assert.equal(got, "mycelium.exe");
});

test("unsupported platform falls back to the PATH command name", () => {
  const got = resolveBinary({
    platform: "sunos",
    arch: "sparc",
    env: {},
    resolver: () => {
      throw new Error("should not resolve an unsupported platform");
    },
  });
  assert.equal(got, "mycelium");
});

test("platformPackage / binaryName mirror the RFC-0110 launcher table", () => {
  assert.equal(platformPackage("linux", "arm64"), `${SCOPE}/mycelium-linux-arm64-gnu`);
  assert.equal(platformPackage("sunos", "sparc"), null);
  assert.equal(binaryName("win32"), "mycelium.exe");
  assert.equal(binaryName("linux"), "mycelium");
});
