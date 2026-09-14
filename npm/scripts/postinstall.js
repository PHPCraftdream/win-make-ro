"use strict";

// Registers the Explorer context menu after a global install. Nothing here is
// allowed to fail the install: a missing registration is recoverable with one
// command, an aborted `npm install -g` is not.
//
// Why it asks for a reinstall first, and what the fallback is for, is written
// down beside REGISTER_COMMANDS.

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const {
  REGISTER_COMMANDS,
  REMOVAL_NOTICE,
  exe,
  isWindows,
  shouldRegister,
} = require("./paths.js");

function main() {
  if (!isWindows()) {
    return;
  }
  if (!fs.existsSync(exe())) {
    console.error("win-make-ro: the packaged executable is missing; nothing was registered.");
    return;
  }
  if (!shouldRegister()) {
    console.log(
      "win-make-ro: installed locally, so the context menu was not registered.\n" +
        "             Run `npx win-make-ro install` to register it for your user."
    );
    return;
  }

  switch (REGISTER_COMMANDS.find(run)) {
    case "reinstall":
      console.log("win-make-ro: context menu registered for the current user.");
      return;
    case "install":
      console.log(
        "win-make-ro: context menu registered, but Explorer was not restarted.\n" +
          "             If it is still running an older copy, run `win-make-ro reinstall`\n" +
          "             from a window without administrator rights."
      );
      return;
    default:
      console.error(
        "win-make-ro: could not register the context menu.\n" +
          "             Run `win-make-ro install` to retry."
      );
  }
}

function run(command) {
  const result = spawnSync(exe(), [command], { stdio: "inherit" });
  return !result.error && result.status === 0;
}

main();
if (isWindows()) {
  // Printed whatever happened above: by the time it matters, npm will not run
  // anything of ours again.
  console.log(REMOVAL_NOTICE);
}
