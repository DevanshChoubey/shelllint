# shellcheck-rust

A Rust port of [ShellCheck](https://github.com/koalaman/shellcheck), aiming
for the same warnings, SC codes, CLI flags and output formats, with lower memory
use and higher speed. Work in progress; nothing is usable yet.

Licensed GPL-3.0, like ShellCheck, because it is a direct port of its source.
The working name is a placeholder.

- `reference/shellcheck`: upstream source, pinned to `e11f964`
- `corpus/`: ShellCheck's own tests extracted to JSON and checked against the real binary
- `tools/`: corpus and manifest scripts
- `docs/phase0.md`: what's been set up and the numbers behind it
