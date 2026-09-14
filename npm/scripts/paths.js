"use strict";

// Where the packaged binaries live, and the rules every lifecycle script
// shares: this package is Windows-only, and registering the shell extension
// is a machine-level side effect that only a global install should cause.

const path = require("node:path");

const VENDOR = path.join(__dirname, "..", "vendor");

function exe() {
  return path.join(VENDOR, "win-make-ro.exe");
}

function isWindows() {
  return process.platform === "win32";
}

/** npm sets this for `npm install -g`; a project-local install must not touch
 * the user's Explorer. `WIN_MAKE_RO_SKIP_REGISTER=1` opts out either way. */
function shouldRegister() {
  if (!isWindows()) return false;
  if (process.env.WIN_MAKE_RO_SKIP_REGISTER === "1") return false;
  return process.env.npm_config_global === "true";
}

module.exports = { VENDOR, exe, isWindows, shouldRegister };
