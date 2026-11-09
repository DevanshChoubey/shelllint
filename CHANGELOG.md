# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Each release also states which upstream ShellCheck commit it ports.

## [Unreleased]

### Added

See [docs/extensions.md](docs/extensions.md) for details. Numbers refer to
ShellCheck issues.

- File selection: `-r` (#143, #1895), `--exclude-path` (#2011, #2411),
  `--changed-since` (#1935), `--stdin-filename` (#2735).
- Output: `-f sarif` (#2405), `-f github`, `-f context` (#3146), `--output`
  (#2348), `--tab-width` (#3454), `--summary` and `--fail-fast` (#2018, #3599),
  `--verbose` (#2228, #3590), `--dump-ast` (#2696), `--list-checks`,
  `--completions` (#309), `--cache-dir` (#2939).
- Shell selection: `--fallback-shell`, `--infer-shell-from-name`,
  `--skip-unsupported-shebang` (#2610, #2971, #2425).
- Directives and rc files: `disable-start`, `disable-end`, `disable-line`;
  `severity`, `external-vars`, `source-functions`, `RCDIR`, `color`,
  `wiki-link-count`; `--strict-rc`; `--source-from-path`.
- Optional checks: `require-strict-mode` (#409), `prefer-local` (#468),
  `arith-errexit` (#2777), `undefined-command` (#2574), `unused-function` (#612).
- An Alpine container image.

### Changed

- `nix-shell` shebangs are understood, and `~` and `$HOME` are expanded in source
  paths. See [docs/divergences.md](docs/divergences.md).

### Fixed

- Scripts that source each other no longer use unbounded time and memory
  (#3442, #2954).

## [0.1.0] - 2026-11-09

Ports ShellCheck at upstream commit `e11f964` (`v0.11.0-171-ge11f964`).

### Added

- A complete port of ShellCheck's parser, analyzer and every registered check
  (`Analytics.hs`, `Checks/Commands.hs`, `Checks/ShellSupport.hs`), including
  control-flow and data-flow analysis.
- The `shelllint` command-line tool, with ShellCheck's flags and the
  `tty`, `gcc`, `json`, `json1`, `checkstyle`, `diff` and `quiet` output
  formats.
- Support for `.shellcheckrc` and `shellcheckrc` lookup, `--rcfile`, `--norc`,
  EditorConfig `shellcheck_*` keys, and in-script directives.
- A conformance harness (`shelllint-conformance`) with the `report` and
  `parity` gates, and pathological-input tests for time and memory.
  2105 of 2105 corpus scripts match the warning codes of upstream ShellCheck.
- Project documentation: README, NOTICE, CONTRIBUTING, Code of Conduct,
  security policy, issue and pull request templates, and CI and release
  workflows.

[Unreleased]: https://github.com/DevanshChoubey/shelllint/compare/v0.1.0...main
[0.1.0]: https://github.com/DevanshChoubey/shelllint/releases/tag/v0.1.0
