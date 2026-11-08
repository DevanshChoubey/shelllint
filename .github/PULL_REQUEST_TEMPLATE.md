## What this changes

<!-- One topic per PR. Link the related issue, for example "Fixes #123". -->

## Upstream fidelity

<!-- Pick one and delete the other. -->

- [ ] This matches upstream ShellCheck at the pinned commit (`reference/shellcheck`).
      Haskell reference: `src/ShellCheck/<File>.hs`, lines ...
- [ ] This diverges from upstream, and the code carries a `// DIVERGENCE:` note that explains why.

For behaviour changes, show the oracle output and ours for the same input:

```
$ printf '%s\n' '...' | docker run --rm -i koalaman/shellcheck:latest -s bash -f gcc -

$ printf '%s\n' '...' | shelllint -s bash -f gcc -

```

## Gates

<!-- Paste the summary lines. Run cargo inside (ulimit -v 8000000; ...). -->

- [ ] `cargo run -q -p shelllint-conformance -- report`: `pass N / fail 0 / ...`
- [ ] `cargo run -q -p shelllint-conformance --features checker -- parity`: no regressions
- [ ] `cargo test --workspace -- --test-threads=2` passes
- [ ] `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean for the files I touched
- [ ] I updated `corpus/parity-baseline.json` only for improvements, and I say so above
- [ ] I added an entry under `Unreleased` in `CHANGELOG.md`, if users can see the change

## Licensing

- [ ] New source files start with the `SPDX-License-Identifier` line and a provenance line.
- [ ] I agree to license this contribution under the project's license, the GNU GPL, version 3 or later (`GPL-3.0-or-later`).
