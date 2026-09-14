#!/usr/bin/env node
"use strict";

// Thin pass-through to the packaged executable. Arguments are handed to
// spawnSync as an array, so quoting and non-ASCII names are left to Node
// rather than reassembled by hand.

const { spawnSync } = require("node:child_process");
const { exe } = require("../scripts/paths.js");

const result = spawnSync(exe(), process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(`win-make-ro: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
