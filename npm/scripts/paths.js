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

/** What postinstall asks the executable to do after a global install.
 *
 * `reinstall` rather than `install`, because npm has just rewritten the DLL
 * that a running Explorer may still have mapped: registering alone would leave
 * the old code in use until the user next logs out. With nothing registered yet
 * it is an ordinary install and leaves the desktop alone.
 *
 * `--here` says out loud that this registers the copy npm just unpacked. It is
 * what passing neither `--here` nor `--to` already means, and saying it means
 * a future change of default cannot silently point this at somebody else's
 * installation directory.
 *
 * No fallback to plain `install`: an elevated shell — ordinary enough for
 * `npm install -g` on Windows — is handled inside the executable, which
 * registers the pair and reports that the restart was skipped rather than
 * failing. What is left over really is failure. */
const REGISTER_ARGV = ["reinstall", "--here"];

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

module.exports = {
  VENDOR,
  REGISTER_ARGV,
  REMOVAL_NOTICE,
  exe,
  isWindows,
  shouldRegister,
};
