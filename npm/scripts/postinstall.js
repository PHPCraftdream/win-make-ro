"use strict";

// Registers the Explorer context menu after a global install. Nothing here is
// allowed to fail the install: a missing registration is recoverable with one
// command, an aborted `npm install -g` is not.
//
// What it asks the executable to do, and why, is written down beside
// REGISTER_ARGV.

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const { REGISTER_ARGV, REMOVAL_NOTICE, exe, isWindows, shouldRegister } = require("./paths.js");

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
  // The executable reports what it did on its own, including whether Explorer
  // was restarted, so there is nothing to add on success.
  const result = spawnSync(exe(), REGISTER_ARGV, { stdio: "inherit" });
  if (result.error || result.status !== 0) {
    console.error(
      "win-make-ro: the context menu was not fully set up.\n" +
        "             Run `win-make-ro reinstall --here` from a window without\n" +
        "             administrator rights, or `win-make-ro install` to register only."
    );
  }
}

main();
if (isWindows()) {
  // Printed whatever happened above: by the time it matters, npm will not run
  // anything of ours again.
  console.log(REMOVAL_NOTICE);
}
