#!/usr/bin/env node
// Mycelium CLI launcher (RFC-0110).
//
// Resolves the prebuilt `mycelium` binary from the matching per-platform
// optionalDependency package and execs it, forwarding argv and the exit code.
// No network, no Rust toolchain required. Works under both Node and Bun.
"use strict";

const { spawnSync } = require("node:child_process");

/** npm scope for the published packages. */
const SCOPE = "@aimasteracc";

/**
 * Map of `${process.platform}-${process.arch}` → platform package suffix.
 * Add entries here (and a build target) to support more platforms — purely
 * additive, never a breaking change.
 */
const PLATFORMS = Object.freeze({
  "darwin-arm64": "mycelium-darwin-arm64",
  "darwin-x64": "mycelium-darwin-x64",
  "linux-x64": "mycelium-linux-x64-gnu",
  "linux-arm64": "mycelium-linux-arm64-gnu",
  "win32-x64": "mycelium-win32-x64",
});

/** The full platform package name for a platform/arch, or null if unsupported. */
function platformPackage(platform, arch) {
  const suffix = PLATFORMS[`${platform}-${arch}`];
  return suffix ? `${SCOPE}/${suffix}` : null;
}

/** The binary file name for a platform (`.exe` on Windows). */
function binaryName(platform) {
  return platform === "win32" ? "mycelium.exe" : "mycelium";
}

/**
 * Resolve the absolute path to the prebuilt binary, or null if the platform is
 * unsupported or its package is not installed. `resolver` is injected for
 * testability (defaults to `require.resolve`).
 */
function resolveBinary(platform, arch, resolver) {
  const pkg = platformPackage(platform, arch);
  if (!pkg) return null;
  try {
    return resolver(`${pkg}/bin/${binaryName(platform)}`);
  } catch {
    return null;
  }
}

const SIGNAL_NUMS = Object.freeze({
  SIGHUP: 1, SIGINT: 2, SIGQUIT: 3, SIGKILL: 9, SIGPIPE: 13, SIGTERM: 15,
});

/** Returns the conventional 128+N exit code for a POSIX signal name. */
function signalToExitCode(signal) {
  return 128 + (SIGNAL_NUMS[signal] ?? 0);
}

function main() {
  const bin = resolveBinary(process.platform, process.arch, require.resolve);
  if (!bin) {
    const key = `${process.platform}-${process.arch}`;
    console.error(`mycelium: no prebuilt binary available for ${key}.`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(", ")}.`);
    console.error(
      "If your platform is missing, install from source: cargo install mycelium-rcig-cli",
    );
    process.exit(1);
  }
  const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
  if (result.error) {
    console.error(`mycelium: failed to launch binary: ${result.error.message}`);
    process.exit(1);
  }
  if (result.signal) {
    process.exit(signalToExitCode(result.signal));
  }
  process.exit(result.status === null ? 1 : result.status);
}

module.exports = { SCOPE, PLATFORMS, platformPackage, binaryName, resolveBinary, signalToExitCode };

if (require.main === module) {
  main();
}
