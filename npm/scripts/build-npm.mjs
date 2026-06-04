#!/usr/bin/env node
// Assemble the publishable npm packages from prebuilt binaries (RFC-0110).
//
// Usage:
//   node npm/scripts/build-npm.mjs --version 0.1.20 --bin-dir <dir> --out <dir>
//
// <bin-dir> must contain one subdirectory per platform key, each holding the
// built `mycelium` binary, e.g.:
//   <bin-dir>/darwin-arm64/mycelium
//   <bin-dir>/win32-x64/mycelium.exe
//
// Produces, under <out>:
//   <out>/mycelium/                          (the universal launcher package)
//   <out>/mycelium-<platform>/               (one per platform, binary inside)
// each with package.json `version` and optionalDependency pins set to --version.
"use strict";

import { mkdir, copyFile, writeFile, chmod, readFile, access } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SCOPE = "@aimasteracc";

// platform key → { os, cpu, exe } (keys mirror bin/mycelium.cjs PLATFORMS).
const TARGETS = {
  "darwin-arm64": { os: "darwin", cpu: "arm64", suffix: "darwin-arm64", exe: false },
  "darwin-x64": { os: "darwin", cpu: "x64", suffix: "darwin-x64", exe: false },
  "linux-x64": { os: "linux", cpu: "x64", suffix: "linux-x64-gnu", exe: false },
  "linux-arm64": { os: "linux", cpu: "arm64", suffix: "linux-arm64-gnu", exe: false },
  "win32-x64": { os: "win32", cpu: "x64", suffix: "win32-x64", exe: true },
};

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    const k = argv[i]?.replace(/^--/, "");
    if (k) args[k] = argv[i + 1];
  }
  if (!args.version) throw new Error("--version is required");
  args["bin-dir"] = args["bin-dir"] || "dist-bin";
  args.out = args.out || "dist-npm";
  return args;
}

async function exists(p) {
  try {
    await access(p);
    return true;
  } catch {
    return false;
  }
}

async function buildPlatformPackage(out, binDir, version, key, t) {
  const pkgName = `${SCOPE}/mycelium-${t.suffix}`;
  const dir = join(out, `mycelium-${t.suffix}`);
  await mkdir(join(dir, "bin"), { recursive: true });
  const binName = t.exe ? "mycelium.exe" : "mycelium";
  const src = join(binDir, key, binName);
  await copyFile(src, join(dir, "bin", binName));
  if (!t.exe) await chmod(join(dir, "bin", binName), 0o755);
  const pkg = {
    name: pkgName,
    version,
    description: `Mycelium CLI prebuilt binary for ${key}.`,
    license: "MIT",
    repository: { type: "git", url: "git+https://github.com/aimasteracc/mycelium.git" },
    os: [t.os],
    cpu: [t.cpu],
    files: ["bin/"],
  };
  await writeFile(join(dir, "package.json"), JSON.stringify(pkg, null, 2) + "\n");
  return pkgName;
}

async function buildMainPackage(out, version, optionalDeps, here) {
  const dir = join(out, "mycelium");
  await mkdir(join(dir, "bin"), { recursive: true });
  await copyFile(join(here, "..", "mycelium", "bin", "mycelium.cjs"), join(dir, "bin", "mycelium.cjs"));
  const base = JSON.parse(await readFile(join(here, "..", "mycelium", "package.json"), "utf8"));
  base.version = version;
  base.optionalDependencies = Object.fromEntries(optionalDeps.map((n) => [n, version]));
  await writeFile(join(dir, "package.json"), JSON.stringify(base, null, 2) + "\n");
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const here = dirname(fileURLToPath(import.meta.url));
  await mkdir(args.out, { recursive: true });

  const optionalDeps = [];
  for (const [key, t] of Object.entries(TARGETS)) {
    const platformDir = join(args["bin-dir"], key);
    if (!(await exists(platformDir))) {
      console.warn(`build-npm: skipping ${key} (no binary at ${platformDir})`);
      continue;
    }
    optionalDeps.push(await buildPlatformPackage(args.out, args["bin-dir"], args.version, key, t));
    console.log(`build-npm: assembled ${SCOPE}/mycelium-${t.suffix}`);
  }
  if (optionalDeps.length === 0) throw new Error("no platform binaries found; nothing to build");
  await buildMainPackage(args.out, args.version, optionalDeps, here);
  console.log(`build-npm: assembled ${SCOPE}/mycelium with ${optionalDeps.length} platform deps @ ${args.version}`);
}

main().catch((e) => {
  console.error(`build-npm: ${e.message}`);
  process.exit(1);
});
