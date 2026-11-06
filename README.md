# shellcheck-rust

**An unofficial, faithful Rust port of [ShellCheck](https://github.com/koalaman/shellcheck): the same warnings, SC codes and CLI, in a single native binary.**

[![CI](https://github.com/DevanshChoubey/shellcheck-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/DevanshChoubey/shellcheck-rust/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/shellcheck-rust.svg)](https://crates.io/crates/shellcheck-rust)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
![Status: pre-release](https://img.shields.io/badge/status-pre--release-orange.svg)

> [!IMPORTANT]
> **This is an unofficial port.** It is not affiliated with, endorsed by, or
> supported by the ShellCheck project or by Vidar Holen. All check logic,
> warning codes and warning messages derive from ShellCheck. The wiki links in
> our output (`https://www.shellcheck.net/wiki/SCxxxx`) point to the
> **upstream ShellCheck wiki**, which documents ShellCheck, not this port.
> Please do not report bugs in this port to the ShellCheck project. Report
> them [here](https://github.com/DevanshChoubey/shellcheck-rust/issues).

## Why this exists

ShellCheck is the standard linter for shell scripts. This project ports its
Haskell source to Rust line by line, without redesigning it. The goals are:

- **Same results.** The same checks, SC codes, messages, directives,
  `.shellcheckrc` handling and output formats, so it can replace ShellCheck in
  existing pipelines without any change to configuration.
- **Easy to ship and embed.** A single native binary with no GHC runtime. A
  Rust codebase also makes a WASM build, an embeddable library and an LSP
  server practical.
- **Speed and memory.** Early measurements are promising but not final. See
  [Benchmarks](#benchmarks).

If ShellCheck already works for you, keep using it. This port sits alongside
the upstream project and does not replace it.

## Status

Pre-release. Every registered check is ported. On warning codes, the port
agrees with the real ShellCheck binary on all of ShellCheck's own test
scripts (see [Compatibility](#compatibility)). There is no tagged release yet,
and the crates are not published.

## Install

Build from source. This needs Rust 1.88 or newer:

```bash
git clone https://github.com/DevanshChoubey/shellcheck-rust
cd shellcheck-rust
cargo install --locked --path crates/cli
```

Once releases exist, you will also be able to install it these ways:

```bash
cargo install --locked shellcheck-rust            # from crates.io
# or download a prebuilt binary (with SHA-256 checksums) from GitHub Releases
```

Homebrew, AUR, Nix, npm and PyPI wrappers, a pre-commit hook, a GitHub Action
and a Docker image are planned. See [docs/launch-plan.md](docs/launch-plan.md).

## Usage

The binary is called `shellcheck-rust` and takes the same flags as
`shellcheck`:

```bash
shellcheck-rust myscript.sh
shellcheck-rust -s bash -f gcc scripts/*.sh
shellcheck-rust -e SC2086,SC2034 -S warning deploy.sh
shellcheck-rust -f diff broken.sh | git apply      # apply the suggested fixes
shellcheck-rust --list-optional
```

- **Output formats** (`-f`): `tty` (the default), `gcc`, `json`, `json1`,
  `checkstyle`, `diff` and `quiet`.
- **Configuration.** Like ShellCheck, it searches for `.shellcheckrc` or
  `shellcheckrc` from the script's directory upwards, then in `$HOME` and the
  XDG config directory. It also supports `--rcfile`, `--norc`, and the
  `shellcheck_*` keys in EditorConfig files.
- **Directives.** `# shellcheck disable=...`, `source=...`, `shell=...`,
  `enable=...` and the other directives work as in ShellCheck.
- Run `shellcheck-rust --help` to see every flag.

### `--fix` (extension)

`--fix` is not in upstream ShellCheck. It applies every auto-fix to the files
in place and then prints what is left in the chosen `-f` format (nothing with
`-f quiet`). It exits 0 if no issues remain and 1 if some do.

```bash
shellcheck-rust --fix deploy.sh lib/*.sh
shellcheck-rust --fix --dry-run deploy.sh   # print the diff (same as -f diff), change nothing
```

- It uses the same fix merging and application as `-f diff`, so one round is
  the same as `-f diff | patch -p1`. If some fixes overlap, the merge drops
  them, so `--fix` repeats until nothing changes. A second run is a no-op.
- Each file is written atomically: a temp file in the same directory is
  renamed over it. Permissions, CRLF line endings and a missing final newline
  are kept, as `patch` keeps them. Files with no fixes are not rewritten.
- A sourced file (with `-x`/`-a`) is fixed only if it is also given on the
  command line. Otherwise it is skipped with a note.
- Standard input (`-`) and non-UTF-8 files are skipped with an error (exit 2).

To use it as a drop-in replacement, symlink it as `shellcheck`:

```bash
ln -s "$(command -v shellcheck-rust)" ~/.local/bin/shellcheck
```

## Compatibility

**Claim:** at upstream commit
[`e11f964`](https://github.com/koalaman/shellcheck/commit/e11f964f6f2cb0009dd259ec33d563c45aa3453d)
(`v0.11.0-171-ge11f964`), **2105 of 2105** corpus scripts produce the same set
of SC warning codes as the real ShellCheck binary.

**How it is measured:**

1. **Corpus.** `tools/extract_corpus.py` extracts every `prop_` unit test from
   ShellCheck's Haskell source into `corpus/tests.jsonl`. The source is
   `reference/shellcheck`, a git submodule pinned to `e11f964`. There are
   2,334 records: per-token checks, whole-script checks, exact-code tests,
   parser tests and unit tests.
2. **Oracle.** `tools/validate_corpus.py` runs every runnable script through the
   real ShellCheck binary and stores the warning codes it reports on each
   record, as `oracle_codes`. The binary comes from the `koalaman/shellcheck`
   Docker image, which reported the same version as the pinned source. The
   stored corpus already holds these results, so the gates do not need Docker.
3. **Unit gate.** `shellcheck-conformance report` runs each ported check
   against its own upstream tests, of the form "this check fires on this
   snippet" or "this check stays silent on that one". Current result: pass
   2105, fail 0.
4. **Parity gate.** `shellcheck-conformance parity` takes each of the 2,105
   scripts that have oracle data and runs it through the complete pipeline:
   the parser, the analyzer, every check, directives and filtering. It then
   compares the resulting set of SC codes with the oracle's.
   `corpus/parity-baseline.json` records which scripts match, and CI fails on
   any regression.
5. **Full parity gate.** `parity --full` compares the exact, ordered `json1`
   output (every comment's code, level, line, column, end position, message
   and fix) with the oracle's, stored in `corpus/oracle-full.jsonl`. Current
   result: 2105 of 2105 identical.

```bash
cargo run -q -p shellcheck-conformance -- report
cargo run -q -p shellcheck-conformance --features checker -- parity
cargo run -q -p shellcheck-conformance --features checker -- parity --full
```

Beyond the corpus, two rounds of differential testing compared the release
binary with the original on tens of thousands of generated, mutated and
real-world scripts, plus the CLI options, the LSP and the MCP server. The
remaining known differences are listed in
[`docs/divergences.md`](docs/divergences.md).

**What this does not prove:**

- The corpus is ShellCheck's own test suite, which is mostly small snippets.
  Differential testing widens the coverage but cannot be exhaustive.

## Benchmarks

<!-- BENCH: fill from docs/benchmarks.md -->

There are no published numbers yet. Early measurements of speed and peak
memory against the Haskell binary are promising but **not final**. Published
benchmarks will state the machine, the inputs, the versions of both binaries
and the exact commands, so anyone can reproduce them.

| Workload | ShellCheck (Haskell) | shellcheck-rust | Ratio |
|---|---|---|---|
| <!-- BENCH: workload --> | <!-- BENCH --> | <!-- BENCH --> | <!-- BENCH --> |

## Differences from ShellCheck

The port is literal by design. It keeps the same algorithms, the same check
order and the same messages. In places where Rust cannot express the Haskell
directly, or where behaviour differs on purpose, the code carries a
`// DIVERGENCE:` comment that explains why. Examples are regex syntax
translations, differences in Unicode character classes, and synthetic AST
nodes that a read-only arena cannot build. To list them all:

```bash
grep -rn 'DIVERGENCE:' crates/
```

Other known differences:

- The binary is called `shellcheck-rust`. `--version` reports both its own
  version and the upstream version it ports, and links to this project.
- `--fix` and `--dry-run` are extensions (see [`--fix`](#--fix-extension)).
- A few edge cases differ where the original crashes or depends on the
  locale (for example invalid UTF-8 in `--files-from`, lone surrogates, an
  out-of-memory exit). [`docs/divergences.md`](docs/divergences.md) lists them.

For the same input and the same upstream version, any output that differs from
ShellCheck's is a bug, unless a `DIVERGENCE:` note explains it. Please report
it with the
[false positive / false negative](.github/ISSUE_TEMPLATE/false-positive-negative.yml)
issue template.

## Architecture

A Cargo workspace whose crates mirror ShellCheck's Haskell modules:

| Crate | Ports | Role |
|---|---|---|
| `crates/core` (`shellcheck-core`) | `AST.hs`, `ASTLib.hs`, `Interface.hs`, `Data.hs`, `Prelude.hs`, `Regex.hs` | The arena AST, shared types, data tables and regex helpers |
| `crates/parser` (`shellcheck-parser`) | `Parser.hs` | The shell parser: turns a script into an AST plus parse diagnostics |
| `crates/analyzer` (`shellcheck-analyzer`) | `Analyzer.hs`, `AnalyzerLib.hs`, `CFG.hs`, `CFGAnalysis.hs` | The analysis framework, the check registry, and control-flow and data-flow analysis |
| `crates/checks` (`shellcheck-checks`) | `Analytics.hs`, `Checks/*.hs` | The checks, one file each, collected into a generated `CHECKS` registry by `build.rs` |
| `crates/checker` (`shellcheck-checker`) | `Checker.hs`, `EditorConfig.hs` | Runs the parser, analyzer and checks, then applies directives, filters and severities |
| `crates/formatter` (`shellcheck-formatter`) | `Formatter/*.hs`, `Fixer.hs` | The output formats and the code that applies fixes |
| `crates/cli` (`shellcheck-rust`) | `shellcheck.hs` | The `shellcheck-rust` binary: flags, rc files and file handling |
| `crates/conformance` | none | The corpus runner: the `report` and `parity` gates and the pathological-input tests |

Other directories:

- `reference/shellcheck`: the upstream source, as a submodule pinned to
  `e11f964`.
- `corpus/`: the extracted tests, the oracle results, the check manifest and
  the parity baseline.
- `tools/`: scripts that extract and validate the corpus and build the manifest.
- `docs/`: the [porting guide](docs/porting-a-check.md), design notes and the
  [launch plan](docs/launch-plan.md).

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md)
first. The two main rules: every behaviour change must either match upstream
ShellCheck or carry a `// DIVERGENCE:` note, and the `report` and `parity`
gates must stay green. Everyone taking part follows the
[Code of Conduct](CODE_OF_CONDUCT.md). To report a security issue, see
[SECURITY.md](SECURITY.md).

## License and credits

shellcheck-rust is a derivative work of ShellCheck, licensed under the
**GNU General Public License, version 3 or (at your option) any later version**
(`GPL-3.0-or-later`, the same terms as upstream; see [LICENSE](LICENSE)). The
[NOTICE](NOTICE) file holds the upstream copyright and lists which files derive
from which upstream sources.

ShellCheck is copyright 2012 and later, Vidar Holen and the ShellCheck
contributors. The design, the checks, the warning texts, the SC code numbers
and the wiki all come from their work. If you find this port useful, the
credit belongs to the ShellCheck project, so please
[star and support upstream](https://github.com/koalaman/shellcheck).
