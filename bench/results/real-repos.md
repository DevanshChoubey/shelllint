# Real-repo benchmark: Haskell ShellCheck vs shellcheck-rust

Generated 2026-11-06T21:10:41Z by `bench/run_real.sh`. Raw data: `bench/results/real-repos.json`.

## Machine

- CPU: 12th Gen Intel(R) Core(TM) i5-12600K (16 logical CPUs), RAM 91.8 GB
- Kernel 7.0.0-43-generic, Ubuntu 26.04.1 LTS
- Haskell: `version: v0.11.0-171-ge11f964` (`koalaman/shellcheck:latest`, static binary)
- Rust: `shellcheck-rust - shell script analysis tool (a Rust port of ShellCheck)
version: 0.0.1 (ports ShellCheck v0.11.0-171-ge11f964)
license: GNU General Public License, version 3
website: https://github.com/DevanshChoubey/shellcheck-rust`, release build (LTO, codegen-units=1), 5619168 bytes
- rustc 1.93.1 (01f6ddf75 2026-02-11) (built from a source tarball), Python 3.14.4
- Load average (1 min) at start 9.71, at end 31.12. Other agents share this machine, so timings carry noise.
- Caps: RLIMIT_AS 6000000 KiB per process, timeout 300s. sequential, one process per file, 3 passes, median of pass totals.
- Flags: `-f json1 --norc -o all` (json1 on stdout, shell from shebang)

## Summary

- 920 files, 122416 lines across 7 repos
- Total wall time (sum of per-repo medians): Haskell 165.71 s, Rust 55.59 s, speedup 3.0x
- Diagnostics identical on 918 of 918 comparable files (100.0%), compared on (line, column, code)

## Per repo

| repo | files | lines | Haskell time (s) | Rust time (s) | speedup | Haskell peak RSS (MB) | Rust peak RSS (MB) | identical files |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| nvm | 438 | 26580 | 35.98 | 14.17 | 2.5x | 3910.7 | 597.9 | 437/437 (100.0%) |
| neofetch | 1 | 11592 | 6.48 | 0.37 | 17.7x | 517.6 | 39.5 | 1/1 (100.0%) |
| acme.sh | 294 | 67183 | 101.13 | 37.10 | 2.7x | 3910.4 | 257.9 | 293/293 (100.0%) |
| bats-core | 69 | 5874 | 17.16 | 1.51 | 11.3x | 62.8 | 9.2 | 69/69 (100.0%) |
| asdf | 28 | 757 | 2.15 | 0.74 | 2.9x | 19.8 | 8.0 | 28/28 (100.0%) |
| rbenv | 31 | 1742 | 0.99 | 0.27 | 3.7x | 22.8 | 8.0 | 31/31 (100.0%) |
| pyenv | 59 | 8688 | 1.82 | 1.44 | 1.3x | 38.7 | 9.0 | 59/59 (100.0%) |

Run statuses that were not `ok` (timeouts, crashes, exit codes other than 0/1):

- nvm hs: {'exit 251': 1}
- acme.sh hs: {'exit 251': 1}

## Disagreements

No diagnostic differences.

Files with any difference (`only_hs` = Haskell only, `only_rs` = Rust only):

- none
