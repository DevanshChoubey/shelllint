// SPDX-License-Identifier: GPL-3.0-or-later
// Loads the wasm build in node and compares `check()` against the native
// CLI's `-f json1` output, byte for byte, for a set of scripts and options.
//
//   scripts/build-wasm.sh                 # produces playground/pkg/
//   cargo build -p shelllint        # native CLI (CARGO_TARGET_DIR respected)
//   node crates/wasm/test/node-compare.mjs
//
// Env: NATIVE_BIN (default: $CARGO_TARGET_DIR or ./target)/debug/shelllint).
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, writeFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { initSync, check, fix, list_checks, version } from "../../../playground/pkg/shelllint_wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
const targetDir = process.env.CARGO_TARGET_DIR ?? join(root, "target");
const nativeBin = process.env.NATIVE_BIN ?? join(targetDir, "debug", "shelllint");

initSync({ module: readFileSync(join(root, "playground/pkg/shellcheck_wasm_bg.wasm")) });

const cases = [
  { name: "unquoted var, sh shebang", script: "#!/bin/sh\necho $1\n" },
  { name: "bash, arrays and [[ ]]", script: "#!/bin/bash\narr=(a b)\n[[ $x == y ]] && echo ${arr[@]}\n", shell: "bash" },
  { name: "no shebang (SC2148)", script: "echo hi\n" },
  { name: "command substitution + loops", script: "#!/bin/sh\nfiles=$(ls)\nfor f in $files; do echo $f; done\n" },
  { name: "tabs and utf-8 columns", script: "#!/bin/bash\n\tfoo=\"héllo\" ; echo $foo\n\t\tcat $bar | grep x\n" },
  { name: "exclude 2086", script: "#!/bin/sh\necho $1 $2\n", exclude: [2086] },
  { name: "include only 2034", script: "#!/bin/sh\nx=1; echo $x $y\n", include: [2034] },
  { name: "severity error", script: "#!/bin/sh\necho $1\n[ $a = b ]\n", severity: "error" },
  { name: "dash dialect", script: "#!/bin/dash\nlocal x=1\n", shell: "dash" },
  { name: "ksh dialect", script: "#!/bin/ksh\n[[ -v x ]] && echo y\n", shell: "ksh" },
  { name: "optional check enabled", script: "#!/bin/sh\nfoo=$(cat x)\necho \"${foo}\"\n", enable: ["require-variable-braces"] },
  { name: "parse error", script: "#!/bin/sh\nif true; then\n" },
  { name: "empty script", script: "" },
  { name: "deep-ish nesting", script: "#!/bin/sh\necho " + "$(".repeat(40) + "true" + ")".repeat(40) + "\n" },
];

// Corpus files small enough to run quickly.
const corpusDir = join(root, "corpus");
for (const f of readdirSync(corpusDir)) {
  const p = join(corpusDir, f);
  if (f.endsWith(".sh") && statSync(p).size < 20000) cases.push({ name: `corpus/${f}`, script: readFileSync(p, "utf8") });
}

function nativeFlags(c) {
  const args = ["-f", "json1"];
  if (c.shell) args.push("-s", c.shell);
  if (c.include) args.push("-i", c.include.join(","));
  if (c.exclude) args.push("-e", c.exclude.join(","));
  if (c.severity) args.push("-S", c.severity);
  if (c.enable) args.push("-o", c.enable.join(","));
  return args;
}

function wasmOptions(c) {
  const o = {};
  for (const k of ["shell", "include", "exclude", "severity", "enable"]) if (c[k] !== undefined) o[k] = c[k];
  return JSON.stringify(o);
}

const dir = mkdtempSync(join(tmpdir(), "sc-wasm-compare-"));
let failures = 0;
let total = 0;
const fail = (msg) => { failures++; console.log("FAIL " + msg); };

console.log(`wasm ShellCheck ${version()}, ${list_checks().length} bytes of check list`);
for (const c of cases) {
  total++;
  // The same path goes to both sides: the json1 output embeds the file name.
  const file = join(dir, `case-${total}.sh`);
  writeFileSync(file, c.script);
  const native = spawnSync(nativeBin, [...nativeFlags(c), file], {
    encoding: "utf8",
    timeout: 60_000,
    maxBuffer: 64 << 20,
    env: { ...process.env, SHELLCHECK_OPTS: "" },
  });
  if (native.error) throw native.error;
  const wasmOut = check(c.script, JSON.stringify({ ...JSON.parse(wasmOptions(c)), filename: file }));
  if (native.stdout !== wasmOut) {
    fail(`${c.name}\n--- native\n${native.stdout}\n--- wasm\n${wasmOut}`);
  } else {
    const n = JSON.parse(wasmOut).comments.length;
    console.log(`ok   ${c.name} (${n} comments)`);
  }
}

// fix(): applying the first fix must match the native diff formatter's
// patched text for a single-diagnostic script.
{
  total++;
  const script = "#!/bin/sh\necho $1\n";
  const fixed = fix(script, "", 0);
  if (fixed !== "#!/bin/sh\necho \"$1\"\n") fail(`fix: got ${JSON.stringify(fixed)}`);
  else console.log("ok   fix: quotes $1 (SC2086)");
}

console.log(`${total - failures}/${total} passed`);
process.exit(failures ? 1 : 0);
