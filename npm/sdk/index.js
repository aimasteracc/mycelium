// @aimasteracc/mycelium-sdk — thin CLI-wrapper SDK for Mycelium (RFC-0111).
//
// Embeds the Mycelium code-intelligence engine in any Node/TS app without a
// Rust toolchain. Locates the prebuilt `mycelium` CLI (RFC-0110), spawns it,
// and returns parsed JSON.
//
//   const { Mycelium } = require("@aimasteracc/mycelium-sdk");
//   const m = new Mycelium({ root: "." });
//   await m.index();
//   const hits = await m.query("#login");
"use strict";

const { Mycelium, MyceliumError } = require("./src/client.js");
const { resolveBinary } = require("./src/resolve-binary.js");

module.exports = { Mycelium, MyceliumError, resolveBinary };
