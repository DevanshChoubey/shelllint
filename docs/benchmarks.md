# Benchmarks

shelllint against the Haskell ShellCheck binary, run on 2026-11-09. Each
process checks one file with `-f json1 --norc -o all`.

Raw results: [real repositories](../bench/results/real-repos.md) and
[synthetic inputs](../bench/results/synthetic.md). To reproduce them, run
`bench/run_published.sh`.

## Setup

| | |
|---|---|
| CPU | Intel Core i5-12600K, pinned to logical CPUs 4-7 |
| Memory limit | 6 GB of real memory, no swap (`docker --memory=6g --memory-swap=6g`) |
| OS | Ubuntu 26.04, Linux 7.0.0 |
| ShellCheck | v0.11.0-171-ge11f964, static binary from `koalaman/shellcheck:latest` |
| shelllint | 0.0.1 at commit 1275c7b, release build with LTO, rustc 1.93.1 |
| Timing | Real repositories: median of 3 passes. Synthetic: median of 5 runs |

Commit 1275c7b is the code that was measured. The machine was not idle: desktop
applications were running, with a load average of about 1 to 2.5 on 16 logical
CPUs.

## Real repositories

920 shell files and 122,416 lines from seven repositories, pinned in
[`bench/repos.toml`](../bench/repos.toml).

Across all seven, ShellCheck took 87.3 s and shelllint 13.9 s, which is 6.3x
faster. That understates the gap, because the 87.3 s includes `nvm.sh`, where
ShellCheck was killed before it finished (see below).

| Repository | Files | Lines | ShellCheck | shelllint | Speedup | ShellCheck peak | shelllint peak |
|---|---:|---:|---:|---:|---:|---:|---:|
| nvm | 438 | 26,580 | 42.41 s | 6.59 s | 6.4x | over 6 GB (killed) | 599 MB |
| neofetch | 1 | 11,592 | 2.12 s | 0.29 s | 7.4x | 518 MB | 40 MB |
| acme.sh | 294 | 67,183 | 38.65 s | 5.89 s | 6.6x | 5,383 MB | 258 MB |
| bats-core | 69 | 5,874 | 1.85 s | 0.44 s | 4.2x | 63 MB | 9.2 MB |
| asdf | 28 | 757 | 0.26 s | 0.11 s | 2.4x | 20 MB | 8.0 MB |
| rbenv | 31 | 1,742 | 0.48 s | 0.16 s | 2.9x | 23 MB | 8.2 MB |
| pyenv | 59 | 8,688 | 1.49 s | 0.43 s | 3.5x | 39 MB | 9.1 MB |

Peak memory is the largest single process in that repository.

`nvm.sh` (6,942 lines) is the one file ShellCheck did not finish. The 6 GB limit
killed it after 35.3 s. shelllint checked the file in 4.2 s using 613 MB. We did
not measure how much memory ShellCheck would need without the limit.

On the other 919 files, both tools reported identical diagnostics, compared on
line, column and code.

## Synthetic inputs

| Input | Lines | ShellCheck | shelllint | Speedup | ShellCheck peak | shelllint peak |
|---|---:|---:|---:|---:|---:|---:|
| `build.sh` | 80 | 0.026 s | 0.007 s | 3.5x | 16.8 MB | 8.1 MB |
| `installer.sh` | 93 | 0.043 s | 0.009 s | 4.7x | 21.9 MB | 7.9 MB |
| `gen_1k.sh` | 1,000 | 0.873 s | 0.080 s | 10.9x | 94.7 MB | 14.6 MB |
| `corpus_concat.sh` | 2,338 | 2.870 s | 0.173 s | 16.6x | 196.6 MB | 27.3 MB |
| `long_case.sh` | 2,811 | 2.753 s | 0.174 s | 15.9x | 1,897.6 MB | 25.8 MB |
| `gen_10k.sh` | 10,000 | 40.409 s | 0.758 s | 53.3x | 645.8 MB | 93.9 MB |
| `patho_1mb_line.sh` | 1 | 1.402 s | 0.170 s | 8.3x | 224.5 MB | 16.0 MB |
| `gen_50k.sh` | 50,000 | over 120 s (timed out) | 4.320 s | not measured | not measured | 441.7 MB |
| `patho_50k_lines.sh` | 50,000 | over 120 s (timed out) | 3.823 s | not measured | not measured | 350.9 MB |
| `patho_nested_5000.sh` | 1 | killed at 6 GB | 0.942 s | not measured | over 6 GB | 189.6 MB |

Generated and pathological files are worst cases that show how each tool scales.
They are not typical scripts, so use the real-repository table to estimate
everyday speed.

Where both tools finished, diagnostics matched exactly on line, column and code.
The last three rows have no comparison because ShellCheck produced no output.

## Limits of these numbers

- They come from one machine, and it was not fully idle.
- Each file runs in its own process. Neither tool's handling of many files in
  one invocation is measured.
- The speedup grows with file size. On small scripts, expect roughly 2x to 5x.
