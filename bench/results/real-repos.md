# Real-repo benchmark: Haskell ShellCheck vs shelllint

Generated 2026-11-09T08:33:31Z by `bench/run_published.sh`. Raw data: `bench/results/real-repos.json`.

## Machine

- CPU: 12th Gen Intel(R) Core(TM) i5-12600K (16 logical CPUs), RAM 91.8 GB
- Kernel 7.0.0-43-generic, Ubuntu 26.04.1 LTS
- Haskell: `version: v0.11.0-171-ge11f964` (`koalaman/shellcheck:latest`, static binary)
- Rust: `shelllint - shell script analysis tool (a Rust port of ShellCheck)
version: 0.0.1 (ports ShellCheck v0.11.0-171-ge11f964)
license: GNU General Public License, version 3
website: https://github.com/DevanshChoubey/shelllint`, release build (LTO, codegen-units=1), 5617448 bytes
- rustc 1.93.1 (01f6ddf75 2026-02-11) (built from a source tarball), Python 3.14.4
- Load average (1 min) at start 1.02, at end 1.23. The machine was not idle, so timings carry noise.
- Caps: cgroup limit 6 GB real memory, no swap (docker --memory=6g --memory-swap=6g), no address-space limit, pinned to CPUs 4-7, timeout 300s. sequential, one process per file, 3 passes, median of pass totals.
- Flags: `-f json1 --norc -o all` (json1 on stdout, shell from shebang)

## Summary

- 920 files, 122416 lines across 7 repos
- Total wall time (sum of per-repo medians): Haskell 87.26 s, Rust 13.91 s, speedup 6.3x
- Diagnostics identical on 919 of 919 comparable files (100.0%), compared on (line, column, code)

## Per repo

| repo | files | lines | Haskell time (s) | Rust time (s) | speedup | Haskell peak RSS (MB) | Rust peak RSS (MB) | identical files |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| nvm | 438 | 26580 | 42.41 | 6.59 | 6.4x | 6119.9 | 598.6 | 437/437 (100.0%) |
| neofetch | 1 | 11592 | 2.12 | 0.29 | 7.4x | 517.7 | 39.7 | 1/1 (100.0%) |
| acme.sh | 294 | 67183 | 38.65 | 5.89 | 6.6x | 5382.7 | 258.1 | 294/294 (100.0%) |
| bats-core | 69 | 5874 | 1.85 | 0.44 | 4.2x | 62.6 | 9.2 | 69/69 (100.0%) |
| asdf | 28 | 757 | 0.26 | 0.11 | 2.4x | 19.6 | 8.0 | 28/28 (100.0%) |
| rbenv | 31 | 1742 | 0.48 | 0.16 | 2.9x | 22.6 | 8.2 | 31/31 (100.0%) |
| pyenv | 59 | 8688 | 1.49 | 0.43 | 3.5x | 38.5 | 9.1 | 59/59 (100.0%) |

Run statuses that were not `ok` (timeouts, crashes, exit codes other than 0/1):

- nvm hs: {'exit 137': 1}

## Disagreements

No diagnostic differences.

Files with any difference (`only_hs` = Haskell only, `only_rs` = Rust only):

- none
