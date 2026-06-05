// Binary resolution for the Mycelium SDK (RFC-0111).
//
// Locates the prebuilt `mycelium` CLI binary (shipped via RFC-0110) without a
// Rust toolchain. Resolution order:
//   1. the MYCELIUM_BIN environment variable (explicit override),
//   2. the matching per-platform optionalDependency package,
//   3. the bare command name, leaving discovery to PATH at spawn time.
//
// All inputs (platform, arch, env, resolver) are injectable so the logic is
// unit-testable with no real binary present.
"use strict";

/** npm scope for the published CLI packages (mirrors the RFC-0110 launcher). */
const SCOPE = "@aimasteracc";

/**
 * `${platform}-${arch}` → platform package suffix. Mirrors the RFC-0110
 * launcher table; adding entries is purely additive.
 */
const PLATFORMS = Object.freeze({
  "darwin-arm64": "mycelium-darwin-arm64",
  "darwin-x64": "mycelium-darwin-x64",
  "linux-x64": "mycelium-linux-x64-gnu",
  "linux-arm64": "mycelium-linux-arm64-gnu",
  "win32-x64": "mycelium-win32-x64",
});

/** The full platform package name, or null if the platform/arch is unsupported. */
function platformPackage(platform, arch) {
  const suffix = PLATFORMS[`${platform}-${arch}`];
  return suffix ? `${SCOPE}/${suffix}` : null;
}

/** The binary file name for a platform (`.exe` on Windows). */
function binaryName(platform) {
  return platform === "win32" ? "mycelium.exe" : "mycelium";
}

/**
 * Resolve the `mycelium` binary path (or a PATH-resolvable command name).
 *
 * @param {object} [opts]
 * @param {string} [opts.platform=process.platform]
 * @param {string} [opts.arch=process.arch]
 * @param {NodeJS.ProcessEnv} [opts.env=process.env]
 * @param {(request: string) => string} [opts.resolver=require.resolve]
 * @returns {string} an absolute path, or the bare command name as a PATH fallback
 */
function resolveBinary(opts = {}) {
  const {
    platform = process.platform,
    arch = process.arch,
    env = process.env,
    resolver = require.resolve,
  } = opts;

  if (env.MYCELIUM_BIN) return env.MYCELIUM_BIN;

  const pkg = platformPackage(platform, arch);
  if (pkg) {
    try {
      return resolver(`${pkg}/bin/${binaryName(platform)}`);
    } catch {
      // Package not installed — fall through to the PATH fallback.
    }
  }

  return binaryName(platform);
}

module.exports = { SCOPE, PLATFORMS, platformPackage, binaryName, resolveBinary };
