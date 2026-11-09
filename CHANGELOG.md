# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Each release also states which upstream ShellCheck commit it ports.

## [Unreleased]

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
