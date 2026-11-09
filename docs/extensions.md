# Extensions beyond ShellCheck

Everything on this page is a shelllint addition. None of it changes the
diagnostics or exit codes of a run that doesn't use it, apart from the cases
listed in [divergences.md](divergences.md). The parity gates check that against
ShellCheck on every change.

## Choosing files

**`-r`, `--recursive`** walks directories among FILES. A file is checked if its
name ends in `.sh`, `.bash`, `.dash`, `.ksh` or `.bats`, or its first line is a
shebang for sh, bash, dash, ksh or busybox (directly or through `env`). Results
are sorted. Symlinks are not followed, and `.git`, `.hg`, `.svn` and
`node_modules` are skipped. Files named explicitly are always checked.

**`--exclude-path=GLOB`** (repeatable) skips matching paths, whether walked or
named. Globs match the path relative to the current directory: `*` stays inside
one path segment, `**` crosses segments, `?` is one character. A glob without
`/` matches any single path component. A glob with `/` also excludes everything
below a matching directory.

**`--changed-since[=REF]`** reports only findings that touch lines changed
relative to REF (default `HEAD`). Untracked files count as entirely changed.
Write the ref with `=`. Outside a git repository, or with an unknown ref, it
exits 2. It can't be used with standard input.

**`--stdin-filename=PATH`** reports the script read from `-` as PATH, in every
format. PATH's directory decides which rc file applies and how `SCRIPTDIR` and
`source` paths resolve. PATH need not exist.

```bash
git show HEAD:deploy/run.sh | shelllint --stdin-filename=deploy/run.sh -
```

## Output

| Format | What it prints |
| --- | --- |
| `-f sarif` | One SARIF 2.1.0 document for all files, for GitHub code scanning and other SARIF tools. |
| `-f github` | GitHub Actions workflow commands, which show as annotations on pull requests. `info` and `style` become `notice`. |
| `-f context` | Each finding with its source lines and a caret under the column. `--context=N` sets the lines on each side (default 5). |

**`--output=FORMAT:PATH`** (repeatable) also writes a report in FORMAT to PATH.
Scripts are checked once however many outputs there are. If a file can't be
created or written, shelllint prints an error and exits 2.

**`--tab-width=N`** expands tabs to N-column stops in the `tty` source echo and
moves the carets to match.

**`--summary`** prints finding counts by severity and the number of files
checked to stderr after the report. **`--fail-fast`** stops after the first file
with findings at or above the active severity, and the report stays valid for
the files processed. **`--verbose`** prints each file and the rc file used for
it to stderr.

**`--dump-ast`** prints each file's parsed syntax tree as compact JSON instead
of the report: a bare document for one file, an array for several. Each node has
`type`, `id`, `start`, `end` and `children`, and `value` when the token has text.
Exit status is 0 if everything parsed, 1 if a file didn't, 2 if a file couldn't
be read.

**`--list-checks[=text|json]`** lists the optional checks. **`--completions=bash|zsh|fish`**
prints a shell completion script generated from the real option table.

```bash
source <(shelllint --completions=bash)
```

**`--cache-dir=PATH`** reuses results for unchanged files. An entry is reused
only if shelllint, the file's bytes, every option that affects results, the rc
file in effect and every file followed through `source` are unchanged. Output is
identical to an uncached run. Standard input and `--fix` bypass the cache.
`undefined-command` results are tied to the value of `$PATH`, so installing a
command without changing `$PATH` won't refresh them.

## Choosing the shell

**`--fallback-shell=SHELL`** assumes SHELL for scripts that name no shell.
**`--infer-shell-from-name`** guesses the shell from names like `.bashrc`.
**`--skip-unsupported-shebang`** treats files with no usable shell as clean.

`--shell` and a `shell=` directive still take precedence.

## Directives and rc files

```bash
# shellcheck disable-start=SC2086,SC2034
cmd $x
# shellcheck disable-end=SC2086
cmd $x # shellcheck disable-line=SC2086
```

`disable-start` suppresses codes from that line until the matching `disable-end`
or the end of the file, and both accept `all`. `disable-line` applies to that
physical line only. These are scanned line by line, so a directive-shaped line
inside a here-doc is also read as a directive.

New `.shellcheckrc` keys:

| Key | Effect |
| --- | --- |
| `severity=warning` | Minimum severity for files the rc covers. `# shellcheck severity=warning` before the first command also works. `--severity` wins, then the directive, then the rc file. |
| `external-vars=A,B,CI_*` | Marks variables as assigned for SC2154. A trailing `*` is a prefix glob. Also `--external-vars=A,B` and a directive. |
| `source-functions=a,b` | Calls to these wrappers are followed like `source ARG` when the first argument is a literal. Also `--source-functions=a,b` and a directive. |
| `safe-vars=A,B` | Tells SC2086, SC2206 and related quoting checks that these variables never contain spaces or glob characters. A later assignment in the script still overrides it. Also `--safe-vars=A,B` and a directive. |
| `strict-env-vars=true` | Stops treating all-uppercase names as environment variables, so unassigned ones get SC2154. Names on the internal list (`HOME`, `PWD` and so on) stay exempt. Also `--strict-env-vars` and a directive. |
| `source-path=RCDIR/sub` | `RCDIR` means the directory holding the rc file. Only a leading component is expanded. |
| `color=auto` | Same as `--color`, taken from the rc file for the first input file. |
| `wiki-link-count=N` | Same as `--wiki-link-count`, taken from the rc file for the first input file. |

**`--modern-dialects`** (rc or directive: `modern-dialects=true`) accepts newer
shell features: `read -d` in sh (POSIX.1-2024), `read -t`, `read -d` and `echo -e`
when the shebang names ash or busybox, and bash 5.1 `assoc=(key value key value)`
without SC2190. Leave it off for scripts that run on older shells.

**`--ignore-directives`** doesn't honour in-file `disable=` or range directives,
which is useful for audits. rc files still apply. **`--report-unused-disables`**
reports each `disable=` code that suppressed nothing as SC9006 (info) on the
directive's line. Codes of optional checks that aren't enabled are never reported.

**`--strict-rc`** turns an unknown rc key, an invalid `shell=`, `color=` or
`wiki-link-count=` value, or an unreadable `--rcfile` into an error with exit 2.

**`--source-from-path`** looks up a bare filename in `source foo.sh` on `$PATH`
when the usual search doesn't find it.

A leading `~`, `$HOME` or `${HOME}` is expanded in `source-path=`, `-P` and
`# shellcheck source=`. An unannotated `source ~/.bashrc` still gives SC1090.

## Optional checks

These are off by default and run only when named. `-o all` leaves them out so
that it keeps matching ShellCheck, so name them even alongside `all`.

| Name | Code | Severity | Reports |
| --- | --- | --- | --- |
| `require-strict-mode` | SC9001 | info | Scripts that don't set errexit, nounset and pipefail. |
| `prefer-local` | SC9002 | style | Function variables never read outside the function. Not for `sh`. |
| `arith-errexit` | SC9003 | warning | A standalone `((..))` that can exit 1 under `set -e`. |
| `undefined-command` | SC9004 | info | Commands not found on this machine's `$PATH`, so results depend on where you run it. |
| `unused-function` | SC9005 | style | Functions never called by name. Off for files with dynamic command names. Leave it off for libraries. |

```bash
shelllint --enable=prefer-local,unused-function script.sh
```

## Containers

An Alpine image with a shell is published alongside the
scratch image, which suits CI systems like GitLab that need a shell:

```yaml
image:
  name: ghcr.io/devanshchoubey/shelllint:alpine
  entrypoint: [""]
```
