"use strict";

// Guards the packaging contract that npm itself cannot enforce. Run with
// `node npm/test/lifecycle.test.js`; CI runs it on every push.

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const { REMOVAL_NOTICE } = require("../scripts/paths.js");

const tests = [];
const test = (name, fn) => tests.push([name, fn]);

// npm 7 and later silently ignore these, so declaring one promises cleanup
// that never happens.
test("declares no uninstall lifecycle scripts", () => {
  for (const hook of ["preuninstall", "postuninstall", "uninstall"]) {
    assert.equal(
      pkg.scripts[hook],
      undefined,
      `${hook} is declared but npm never runs it; the cleanup would be a lie`
    );
  }
});

test("tells the user how to remove the registration", () => {
  assert.match(REMOVAL_NOTICE, /win-make-ro uninstall/);
  assert.match(REMOVAL_NOTICE, /npm uninstall -g win-make-ro/);
  assert.ok(
    REMOVAL_NOTICE.indexOf("win-make-ro uninstall") <
      REMOVAL_NOTICE.indexOf("npm uninstall -g win-make-ro"),
    "unregistering has to come first, while the executable still exists"
  );
});

test("every declared script exists", () => {
  for (const command of Object.values(pkg.scripts)) {
    const file = command.replace(/^node\s+/, "");
    assert.ok(fs.existsSync(path.join(root, file)), `${file} is missing`);
  }
  assert.ok(fs.existsSync(path.join(root, pkg.bin["win-make-ro"])));
});

test("publishes the binaries and nothing private", () => {
  assert.deepEqual(pkg.os, ["win32"]);
  assert.deepEqual(pkg.cpu, ["x64"]);
  for (const entry of ["bin/", "scripts/", "vendor/"]) {
    assert.ok(pkg.files.includes(entry), `${entry} is not published`);
  }
  assert.ok(!pkg.files.includes("test/"), "the tests are not part of the package");
});

// A failed registration must never fail `npm install`.
test("postinstall survives a missing executable", () => {
  const result = spawnSync(process.execPath, [path.join(root, "scripts", "postinstall.js")], {
    env: { ...process.env, npm_config_global: "true", WIN_MAKE_RO_SKIP_REGISTER: "1" },
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  if (process.platform === "win32") {
    assert.match(result.stdout, /win-make-ro uninstall/);
  }
});

let failed = 0;
for (const [name, fn] of tests) {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (err) {
    failed += 1;
    console.error(`FAIL ${name}\n     ${err.message}`);
  }
}
console.log(`${tests.length - failed}/${tests.length} passed`);
process.exit(failed === 0 ? 0 : 1);
