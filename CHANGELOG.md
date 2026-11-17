# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Each release also states which upstream ShellCheck commit it ports.

## [Unreleased]

### Added

- zsh support (experimental). zsh scripts are parsed and checked instead of getting
  SC1071. The analysis follows `setopt`, `unsetopt`, `emulate` and `options[]`, and
  existing checks fire only when their warning is true in zsh. For example, SC2086 is
  quiet for `$x` unless `shwordsplit` is on or the script writes `${=x}`. No zsh-only
  checks are added, and output for other shells is unchanged. `-r` now also picks up
  `.zsh` and `.zsh-theme` files and zsh shebangs. See
  [docs/extensions.md](docs/extensions.md#zsh).

## [0.4.0] - 2026-11-17

Ports ShellCheck at upstream commit `e11f964` (`v0.11.0-171-ge11f964`), the same
commit as 0.3.0.

### Added

- Bash version targeting: `--bash-version=3.2`, the rc key `bash-version=3.2` or
  `# shellcheck bash-version=3.2`. Bash scripts then get a warning for each feature
  newer than the target, such as `declare -A`, `mapfile`, `${v,,}` or `${v@Q}`,
  with one code per feature (SC9100 to SC9137). Valid targets are the bash releases
  3.0 to 5.3. The flag wins over the directive, which wins over the rc key. Other
  shells ignore it, and with no target set output is unchanged. See
  [docs/extensions.md](docs/extensions.md#bash-version-targeting).

## [0.3.0] - 2026-11-15

Ports ShellCheck at upstream commit `e11f964` (`v0.11.0-171-ge11f964`), the same
commit as 0.2.1.

### Added

- Jinja template mode (#1306): `--template=jinja`, the rc key `template=jinja` or
  `# shellcheck template=jinja` before the first command, and automatically for
  `*.j2`, `*.jinja` and `*.jinja2` files. `{{ }}`, `{% %}` and `{# #}` markup is
  blanked before parsing with every position kept, so templates no longer give
  SC1083, SC1054 or SC1009 for their markup. Other files are unchanged.

### Fixed

These change default output in the cases described. See
[docs/divergences.md](docs/divergences.md).

- SC2329 no longer reports a function named in a `trap` action, such as
  `trap cleanup EXIT` or `trap 'quit; exit' EXIT` (#2542).
- SC2034 no longer reports namerefs declared with `declare -n`, `local -n` or
  `typeset -n`, or a literal variable they point to (#817).
- SC2119 and SC2120 no longer fire for functions defined in a sourced file (#1679).
- `source ~/file` is looked up like `source "$HOME/file"` instead of giving
  SC1090, so `-x -P "$HOME"` follows it (#1845).

## [0.2.1] - 2026-11-13

Ports ShellCheck at upstream commit `e11f964` (`v0.11.0-171-ge11f964`), the same
commit as 0.2.0.

### Added

- `shelllint_checker::with_check_stack`: runs a closure on one big-stack thread,
  and `check_script` calls inside it reuse that thread instead of starting one
  each. Use it around a loop that checks many scripts.

### Changed

- Faster on many small scripts: the `set -e` / `set -a` lookups no longer
  compile a regex for every script, and the CLI checks all its files on one
  big-stack thread. Output is unchanged.

## [0.2.0] - 2026-11-09

Ports ShellCheck at upstream commit `e11f964` (`v0.11.0-171-ge11f964`), the same
commit as 0.1.0.

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
- `--ignore-directives` and `--report-unused-disables` (#1417, #645).
- `--modern-dialects` relaxes dialect warnings (#3099, #3062, #2393).
- `safe-vars` lists variables the quoting checks can trust (#587), and
  `strict-env-vars` reports unassigned uppercase variables (#930).
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

[Unreleased]: https://github.com/DevanshChoubey/shelllint/compare/v0.4.0...main
[0.4.0]: https://github.com/DevanshChoubey/shelllint/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/DevanshChoubey/shelllint/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/DevanshChoubey/shelllint/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/DevanshChoubey/shelllint/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/DevanshChoubey/shelllint/releases/tag/v0.1.0
