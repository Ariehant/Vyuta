// Loader for the Vyuta probe-rs Neon addon.
//
// `npm run build` compiles the Rust cdylib and copies it to `index.node`
// alongside this file. The VS Code extension `require()`s this module to call
// into the in-process debug bridge.
"use strict";

const path = require("node:path");

let addon;
try {
  addon = require(path.join(__dirname, "index.node"));
} catch (err) {
  throw new Error(
    "vyuta probe-rs-extension native addon not built. Run `npm run build` " +
      "in rust/probe-rs-extension first.\nUnderlying error: " + err.message
  );
}

module.exports = {
  /** @returns {string} build identifier */
  hello: addon.hello,
  /** @returns {Array<object>} attached debug probes (empty if none) */
  listProbes() {
    return JSON.parse(addon.listProbes());
  },
  /** @returns {string[]} names of chip families known to probe-rs */
  listChipFamilies() {
    return JSON.parse(addon.listChipFamilies());
  },
};
