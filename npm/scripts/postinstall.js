"use strict";

// Registers the Explorer context menu after a global install. Nothing here is
// allowed to fail the install: a missing registration is recoverable with one
// command, an aborted `npm install -g` is not.

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const { exe, isWindows, shouldRegister } = require("./paths.js");

if (!isWindows()) {
  process.exit(0);
}

if (!fs.existsSync(exe())) {
  console.error("win-make-ro: the packaged executable is missing; nothing was registered.");
  process.exit(0);
}

if (!shouldRegister()) {
  console.log(
    "win-make-ro: installed locally, so the context menu was not registered.\n" +
      "             Run `npx win-make-ro install` to register it for your user."
  );
  process.exit(0);
}

const result = spawnSync(exe(), ["install"], { stdio: "inherit" });
if (result.error || result.status !== 0) {
  console.error(
    "win-make-ro: could not register the context menu.\n" +
      "             Run `win-make-ro install` to retry."
  );
  process.exit(0);
}
console.log("win-make-ro: context menu registered for the current user.");
