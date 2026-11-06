# Contributing to shellcheck-rust

Thank you for helping. This project is a **faithful port** of
[ShellCheck](https://github.com/koalaman/shellcheck), so most work here is
about matching upstream exactly, not about designing new behaviour. Please read
this whole page before you open a pull request.

shellcheck-rust is not affiliated with the ShellCheck project. Report problems
with this port here, not upstream.

## The one rule

> **Every behaviour change must match upstream ShellCheck at the pinned commit,
> or carry a `// DIVERGENCE:` note.**

The pinned commit is `e11f964`, in the submodule `reference/shellcheck`.

- "Behaviour" covers which warnings fire, their SC codes, their positions,
  message text, severities, fixes, directive handling, configuration handling,
  CLI flags and output formats.
- If the port differs from the Haskell, put a `// DIVERGENCE:` comment at the
  place where it differs. Say what differs and why, for example "Rust's regex
  crate has no lookahead, so ...". Reviewers will ask for one.
- New checks, new warnings and "improvements" to upstream messages are out of
  scope. Propose those to ShellCheck. Once they land upstream, we port them.
- A fix for a bug that upstream also has is a divergence too. Prefer reporting
  it upstream and matching upstream until it is fixed there.

## Getting set up

```bash
git clone https://github.com/DevanshChoubey/shellcheck-rust
cd shellcheck-rust
git submodule update --init      # the upstream Haskell source, for reading
cargo build
```

You do not need the submodule to build or test, because the corpus is
committed. You need it to read the Haskell you are porting, and to regenerate
the corpus.

## Testing with a memory cap

Tests that run the full checker do so on a thread with a **512 MB stack**
(`CHECK_STACK_BYTES` in `crates/checker`), because the parser is recursive,
just like the Haskell. Many such threads in parallel can exhaust the memory of
a laptop. Cap the address space and limit the number of test threads:

```bash
(ulimit -v 8000000; cargo test --workspace -j 4 -- --test-threads=2)
```

`ulimit -v` takes kilobytes, so `8000000` is about 8 GB. The stack is
reserved address space and mostly never touched, so if it fails with
`failed to spawn` or out-of-memory errors, lower `--test-threads` before you
raise the cap. Use the same subshell form for `cargo run` of the conformance
gates.

## The conformance gates

Every pull request must keep these green. CI runs them on Linux.

| Gate | Command | What it checks |
|---|---|---|
| **report** | `cargo run -q -p shellcheck-conformance -- report` | Each ported check against its own upstream `prop_` tests. Must print `fail 0`. |
| **parity** | `cargo run -q -p shellcheck-conformance --features checker -- parity` | Runs the full pipeline on each corpus script and compares its SC code set with the stored output of real ShellCheck. Fails on any regression against `corpus/parity-baseline.json`. |
| **pathological** | `cargo test -p shellcheck-conformance --test pathological -- --test-threads=2` | Huge or deeply nested inputs must finish in under 10 s and stay under a peak RSS limit. Linux only, because it reads `/proc`. |

Useful variants:

```bash
# One check, as in the porting guide (always pass --module)
cargo run -q -p shellcheck-conformance -- report --module Analytics.hs --target checkEchoWc

# List every parity mismatch
cargo run -q -p shellcheck-conformance --features checker -- parity --verbose
```

When parity improves, for example after a fix that makes a previously
mismatching script match, update the baseline in the same pull request and
say so in the description:

```bash
cargo run -q -p shellcheck-conformance --features checker -- parity --update-baseline
```

Never update the baseline to hide a regression.

## Porting or fixing a check

Follow **[docs/porting-a-check.md](docs/porting-a-check.md)**. It covers the
one-file-per-check layout, `CheckEntry` registration, how Haskell idioms map
to Rust, shared helpers, synthetic tokens and the per-check gate. The pilots
it lists are worked examples.

To fix a check that produces wrong output:

1. Reproduce the problem with the oracle (see below) and with
   `shellcheck-rust`.
2. Find the Haskell definition in `reference/shellcheck/src/ShellCheck/`. The
   header of the Rust file names the source file and the line span.
3. Make the Rust match the Haskell. If the corpus did not catch the bug, add a
   regression test next to the check or in the crate's `tests/` directory.
4. Run the gates.

## The oracle (real ShellCheck via Docker)

The real ShellCheck binary decides how the port should behave on inputs that
the unit tests do not cover:

```bash
printf '%s\n' 'echo $foo' | docker run --rm -i koalaman/shellcheck:latest -s bash -f gcc -
printf '%s\n' 'echo $foo' | shellcheck-rust -s bash -f gcc -
```

- Compare with `-f json1` when you need exact positions and fixes.
- Check that the image's version matches the pinned commit:
  `docker run --rm koalaman/shellcheck:latest --version`. The `latest` tag
  moves. If it has moved past `e11f964`, a difference may come from new
  upstream behaviour and not from a bug in the port.
- Where an upstream unit test and the oracle disagree, the unit test wins for
  the `report` gate. The oracle wins for end-to-end behaviour.
- You only need Docker to investigate or to regenerate the corpus
  (`tools/validate_corpus.py`). The gates use the stored oracle results.

## Code style

- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`
  should be clean on the code you touch. CI enforces both.
- No `unsafe`. No new dependencies without discussing them first in an issue.
- Port literally. Keep the Haskell names (in snake_case) and the Haskell order,
  so that a reviewer can read the two side by side.
- Every new source file starts with an SPDX line and a provenance line:

  ```rust
  // SPDX-License-Identifier: GPL-3.0-or-later
  // Port of ShellCheck src/ShellCheck/Analytics.hs `checkFoo`, lines 100-120 (e11f964)
  ```

  For a file with no upstream source, write `; no Haskell counterpart` in
  place of the line span. (Older files still say `GPL-3.0-only`. They will be
  updated in one mechanical sweep, so do not change them in your pull request.)

## Pull requests

- Keep each pull request to one topic: one check, one bug, or one feature.
- Fill in the pull request template. Include the gate output, and for any
  behaviour change, the oracle output.
- Write commit messages as `area: summary`, for example
  `checks: fix SC2086 inside arithmetic`.
- Add a line to the `Unreleased` section of [CHANGELOG.md](CHANGELOG.md) for
  any change that users can see.

## Tracking upstream

When upstream ShellCheck changes, we move the submodule to a new commit and
port the diff. Do this in a dedicated pull request: bump the submodule,
regenerate the corpus (see `docs/phase0.md`), port the changed checks and
update the `(e11f964)` provenance markers. Do not mix it with other changes.

## Licensing of contributions

shellcheck-rust is a derivative work of ShellCheck and is licensed under the
GNU GPL, version 3 or (at your option) any later version (`GPL-3.0-or-later`,
the same terms as upstream; see [LICENSE](LICENSE) and [NOTICE](NOTICE)). By
submitting a contribution, you agree to license it under the same terms. You
keep your copyright.

## Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
Please be respectful towards upstream as well. ShellCheck's maintainers do not
support this port, so do not send them its issues.
