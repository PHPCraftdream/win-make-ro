"use strict";

// Removes the registry entries before the files go away, so nothing is left
// pointing at a deleted DLL. Like the install side, this never fails the
// command it runs under.

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const { exe, isWindows } = require("./paths.js");

if (!isWindows() || !fs.existsSync(exe())) {
  process.exit(0);
}

const result = spawnSync(exe(), ["uninstall"], { stdio: "inherit" });
if (result.error || result.status !== 0) {
  console.error(
    "win-make-ro: could not remove the context menu registration.\n" +
      "             Run `regsvr32 /u ro_shellext.dll` or re-register and retry."
  );
}
process.exit(0);
