#!/usr/bin/env node
// Thin launcher: finds the prebuilt binary shipped in the matching
// @shellcheck-rust/<platform>-<arch> optional dependency and runs it with the
// caller's arguments, stdio and exit status.
"use strict";

const { spawnSync } = require("node:child_process");

const PACKAGES = {
  "linux-x64": "@shellcheck-rust/linux-x64",
  "linux-arm64": "@shellcheck-rust/linux-arm64",
  "darwin-x64": "@shellcheck-rust/darwin-x64",
  "darwin-arm64": "@shellcheck-rust/darwin-arm64",
  "win32-x64": "@shellcheck-rust/win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PACKAGES[key];
if (!pkg) {
  console.error(
    `shellcheck-rust: no prebuilt binary for ${key}. ` +
      "Build from source instead: cargo install --git https://github.com/OWNER/shellcheck-rust shellcheck-rust"
  );
  process.exit(1);
}

const exe = process.platform === "win32" ? "shellcheck-rust.exe" : "shellcheck-rust";
let binary;
try {
  binary = require.resolve(`${pkg}/bin/${exe}`);
} catch (err) {
  console.error(
    `shellcheck-rust: the binary package ${pkg} is not installed. ` +
      "Reinstall without --omit=optional / --no-optional."
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`shellcheck-rust: failed to start ${binary}: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  // Re-raise the signal so the parent shell sees the same termination.
  process.kill(process.pid, result.signal);
}
process.exitCode = result.status === null ? 1 : result.status;
