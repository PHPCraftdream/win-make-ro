"use strict";

// Where the packaged binaries live, and the rules every lifecycle script
// shares: this package is Windows-only, and registering the shell extension
// is a machine-level side effect that only a global install should cause.

const path = require("node:path");

const VENDOR = path.join(__dirname, "..", "vendor");

/** npm 7 and later never run uninstall scripts, so the registry entries this
 * package writes cannot be cleaned up on the way out. The order below is the
 * only one that leaves nothing behind, and it is printed at install time so
 * it is known before it is needed.
 * See https://docs.npmjs.com/cli/v11/using-npm/scripts/#a-note-on-a-lack-of-npm-uninstall-scripts */
const REMOVAL_NOTICE =
  "To remove win-make-ro, unregister first, then delete the package:\n" +
  "    win-make-ro uninstall\n" +
  "    npm uninstall -g win-make-ro\n" +
  "npm cannot do the first step for you. If the package is already gone, see\n" +
  "the Removal section of the README for the registry keys to delete.";

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

module.exports = { VENDOR, REMOVAL_NOTICE, exe, isWindows, shouldRegister };
