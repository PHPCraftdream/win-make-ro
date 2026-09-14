"use strict";

// Guards the packaging contract that npm itself cannot enforce. Run with
// `node npm/test/lifecycle.test.js`; CI runs it on every push.

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const root = path.join(__dirname, "..");
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const { REGISTER_ARGV, REMOVAL_NOTICE } = require("../scripts/paths.js");

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
});

// Node replaces an unpaired surrogate with U+FFFD while building process.argv,
// so a JS launcher cannot forward a file name that contains one — it would
// hand the operation a different path. The shim npm writes for a native
// executable passes the command line through untouched.
test("the command is the native executable, not a JS launcher", () => {
  const entry = pkg.bin["win-make-ro"];
  assert.equal(entry, "vendor/win-make-ro.exe", "the bin entry must be the packaged exe");
  assert.ok(!entry.endsWith(".js"), "a JS launcher loses unpaired surrogates in argv");
  assert.ok(pkg.files.includes("vendor/"), "the executable has to be published");
});

test("publishes the binaries and nothing private", () => {
  assert.deepEqual(pkg.os, ["win32"]);
  assert.deepEqual(pkg.cpu, ["x64"]);
  for (const entry of ["scripts/", "vendor/"]) {
    assert.ok(pkg.files.includes(entry), `${entry} is not published`);
  }
  assert.ok(!pkg.files.includes("test/"), "the tests are not part of the package");
  assert.ok(!pkg.files.includes("bin/"), "there is no JS launcher to publish");
});

// npm hides the output of scripts that succeed, so the install-time notice
// cannot be the only place this is written down.
test("the README documents the removal order", () => {
  const readme = fs.readFileSync(path.join(root, "..", "README.md"), "utf8");
  const unregister = readme.indexOf("win-make-ro uninstall");
  const remove = readme.indexOf("npm uninstall -g win-make-ro");
  assert.ok(unregister !== -1, "the README does not mention `win-make-ro uninstall`");
  assert.ok(remove !== -1, "the README does not mention `npm uninstall -g win-make-ro`");
  assert.ok(unregister < remove, "unregistering has to be documented first");
  assert.match(readme, /--foreground-scripts/, "the README does not say how to see the notice");
});

/** Runs the real postinstall.js with the operating system replaced.
 *
 * The script and paths.js are the ones that ship; only what they reach for —
 * spawning, the file system, the platform, the environment and the console —
 * is substituted. Nothing spawns the packaged executable, which would register
 * the extension and restart the tester's Explorer. */
function runPostinstall({
  platform = "win32",
  env = {},
  exeExists = true,
  spawn = () => ({ status: 0 }),
}) {
  const calls = [];
  const out = [];
  const errs = [];
  const scripts = path.join(root, "scripts");
  const load = (file) => {
    const module = { exports: {} };
    const context = vm.createContext({
      module,
      exports: module.exports,
      __dirname: scripts,
      process: { platform, env },
      console: { log: (m) => out.push(String(m)), error: (m) => errs.push(String(m)) },
      require: (id) => {
        if (id === "./paths.js") return load("paths.js");
        if (id === "node:path") return path;
        if (id === "node:fs") return { existsSync: () => exeExists };
        if (id === "node:child_process") {
          return {
            spawnSync: (file, argv) => {
              calls.push({ file, argv });
              return spawn(file, argv);
            },
          };
        }
        throw new Error(`postinstall reached for an unexpected module: ${id}`);
      },
    });
    vm.runInContext(fs.readFileSync(path.join(scripts, file), "utf8"), context, {
      filename: file,
    });
    return module.exports;
  };
  load("postinstall.js");
  return { calls, out: out.join("\n"), errs: errs.join("\n") };
}

// An upgrade rewrites the DLL a running Explorer still has mapped, so plain
// registration would leave the old code in use until the next logout. And the
// destination has to be said out loud: a `reinstall` that ever went back to
// inferring one could point npm's files at a Scoop or hand-made installation.
test("a global install registers this copy and asks for the restart", () => {
  assert.deepEqual(REGISTER_ARGV, ["reinstall", "--here"]);
  const { calls, errs } = runPostinstall({ env: { npm_config_global: "true" } });
  assert.equal(calls.length, 1, "the executable was not asked to do anything");
  // Copied out of the script's own realm, where Array is a different class.
  assert.deepEqual(Array.from(calls[0].argv), ["reinstall", "--here"]);
  assert.match(calls[0].file, /win-make-ro\.exe$/);
  assert.equal(errs, "", "a successful registration has nothing to complain about");
});

// The executable reports a skipped restart as success, so a non-zero exit is
// real failure and the advice has to be something the user can act on.
test("a registration that fails says what to run, and never throws", () => {
  for (const spawn of [() => ({ status: 1 }), () => ({ error: new Error("ENOENT") })]) {
    const { calls, errs } = runPostinstall({ env: { npm_config_global: "true" }, spawn });
    assert.equal(calls.length, 1);
    assert.match(errs, /reinstall --here/);
    assert.match(errs, /administrator rights/);
  }
});

test("nothing is spawned when it must not be", () => {
  const cases = [
    ["a project-local install", { env: {} }, /npx win-make-ro install/],
    ["an explicit opt-out", { env: { npm_config_global: "true", WIN_MAKE_RO_SKIP_REGISTER: "1" } }],
    ["a missing executable", { env: { npm_config_global: "true" }, exeExists: false }],
    ["another platform", { platform: "linux", env: { npm_config_global: "true" } }],
  ];
  for (const [name, options, expected] of cases) {
    const { calls, out, errs } = runPostinstall(options);
    assert.equal(calls.length, 0, `${name} reached for the executable`);
    if (expected) assert.match(out + errs, expected, name);
  }
});

test("the removal notice is printed on Windows and only there", () => {
  const windows = runPostinstall({ env: { npm_config_global: "true" } });
  assert.match(windows.out, /npm uninstall -g win-make-ro/);
  const other = runPostinstall({ platform: "linux", env: { npm_config_global: "true" } });
  assert.doesNotMatch(other.out, /npm uninstall -g win-make-ro/);
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
