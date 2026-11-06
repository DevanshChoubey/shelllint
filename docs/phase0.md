# Phase 0: groundwork

Status as of 2026-11-06.

## Reference and oracle

- `reference/shellcheck` is a git submodule pinned to upstream commit `e11f964`
  (2026-11-05, `v0.11.0-171-ge11f964`).
- The oracle is the Docker image `koalaman/shellcheck:latest`, which reports the
  exact same version, so the source we port and the binary we compare against match.

```bash
printf 'echo $foo\n' | docker run --rm -i koalaman/shellcheck:latest -s bash -f json1 -
```

## Test corpus (`corpus/`)

Built by `tools/extract_corpus.py`, which parses every `prop_` test in the Haskell source.

| Kind | Count | Meaning |
|---|---|---|
| `node` | 1,386 | `verify` / `verifyNot`: one per-token check fires or stays silent |
| `tree` | 377 | `verifyTree` / `verifyNotTree`: one whole-script check |
| `codes` | 27 | `verifyCodes`: exact SC codes expected |
| `parse` | 315 | `isOk` / `isWarning` / `isNotOk` on a parser function |
| `unit` | 225 | Tests of internal helpers and end-to-end `Checker.hs` tests; kept as source |
| `unparsed` | 4 | Parser tests that use combinator expressions; kept as source |

`tools/validate_corpus.py` ran every runnable script through the oracle and
recorded `oracle_codes` on each record. Of the 1,576 node/tree/codes tests
whose check has known SC codes, 1,574 (99.9%) agree with the oracle. The 2
disagreements (`checkShebang`, `checkMultipleBangs`) are listed in
`corpus/validation.json`. 214 tests target checks whose codes the static scan
could not see; the manifest fills those in from oracle results.

Bashism tests run with `-s sh`; all other scripts without a shebang run with `-s bash`.

## Task manifest (`corpus/tasks.json`)

Built by `tools/build_manifest.py` from ShellCheck's registration lists
(`nodeChecks`, `treeChecks`, `optionalChecks`, `commandChecks`, ShellSupport `checks`).

- 186 registered checks, covering 1,696 tests.
- Grouped into **160 tasks** of at most ~20 Haskell lines each (larger checks stand alone).
- 8 tasks need the control-flow/data-flow layer (`phase: "cfg"`) and wait for it.
- Every check has a known SC code list.

Correction to earlier estimates: the earlier "~275 checks" counted multi-equation
definitions more than once. The real number of registered checks is 186.

## Not done yet (Phase 1)

The Rust workspace is only a stub binary. Before fan-out we still need the AST
types, the parser, the analyzer helpers (ASTLib/AnalyzerLib), the check
registration API, the CLI, output formats, directives and `.shellcheckrc`.

## Regenerating

```bash
python3 -I tools/extract_corpus.py reference/shellcheck/src/ShellCheck corpus
python3 -I tools/validate_corpus.py corpus
python3 -I tools/build_manifest.py reference/shellcheck/src/ShellCheck corpus corpus/tasks.json
```
