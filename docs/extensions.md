# Extensions beyond ShellCheck

Everything on this page is a shelllint addition. None of it changes the
diagnostics or exit codes of a run that doesn't use it, apart from the cases
listed in [divergences.md](divergences.md). The parity gates check that against
ShellCheck on every change.

## Choosing files

**`-r`, `--recursive`** walks directories among FILES. A file is checked if its
name ends in `.sh`, `.bash`, `.dash`, `.ksh`, `.zsh`, `.zsh-theme` or `.bats`, or its
first line is a shebang for sh, bash, dash, ksh, zsh or busybox (directly or through
`env`). Results
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

## Templates

**`--template=jinja`** (rc key `template=jinja`, or `# shellcheck template=jinja`
before the first command) checks a Jinja template as shell. It is on
automatically for files named `*.j2`, `*.jinja` or `*.jinja2`, and off for every
other file, so ordinary scripts get the same output as before.

```bash
shelllint --template=jinja deploy.sh
shelllint deploy.sh.j2
```

The rc key is read by the command-line tool (including with `--fix`). The
library, the language server and the MCP server use the option, the directive or
the file name.

Before parsing, the markup is blanked out and every character is kept, so line
and column numbers match your file, also after non-ASCII text inside the markup.
A name like `deploy.bash.j2` is judged by the name without `.j2`, `.jinja` or
`.jinja2` when the shell is picked from the file name.

- `{{ expr }}` becomes one `_` per character, so `NAME={{ app }}` is an
  assignment of a plain word.
- `{% stmt %}` and `{# comment #}` become spaces. Newlines inside them stay.
- `{% raw %}...{% endraw %}` content is left as written. The tags are blanked.
- Files followed through `source` are blanked the same way when the checked file
  is a template, or when their own name ends in `.j2`, `.jinja` or `.jinja2`.
- A `--fix` that would rewrite text inside the markup is skipped.
- An unterminated `{{`, `{%` or `{#` is blanked to the end of the file.

The limits:

- Templates aren't rendered. Every branch of an `{% if %}` and every `{% for %}`
  body is checked as if all of them were present, so you can get a duplicate
  definition or unreachable code warning where the real branches exclude each
  other.
- `{{ }}` is replaced even inside single quotes, because the template renders
  before the shell runs.
- shelllint can't see what a placeholder expands to, so it doesn't check the
  shell that the output becomes.
- `${#var}` contains `{#`, which Jinja also reads as a comment start, so
  shelllint blanks from there. Wrap such code in `{% raw %}`.
- Only Jinja is supported.
- The directive is read only from the leading comment lines. A `{% %}` line
  before it stops the scan.

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
| `bash-version=3.2` | Warns about bash features newer than this release (SC9100 to SC9137). Also `--bash-version=3.2` and a directive. See [Bash version targeting](#bash-version-targeting). |
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

## Bash version targeting

Scripts often have to run on an old bash, such as the 3.2 that macOS ships as
`/bin/bash`, but get written and tested on bash 5. Name the oldest bash the script
must run on, and shelllint warns about every feature newer than that:

```bash
shelllint --bash-version=3.2 script.sh
```

The setting is `--bash-version=X.Y`, `bash-version=X.Y` in `.shellcheckrc`, or
`# shellcheck bash-version=X.Y` in a file. The flag wins, then the directive,
then the rc file. A directive covers the whole file.

- Values are real bash releases: 3.0 3.1 3.2 4.0 4.1 4.2 4.3 4.4 5.0 5.1 5.2 5.3.
  A bare `4` means 4.0.
- It's off until a target is set. With no target, output is unchanged.
- It applies only when the shell is bash (a bash shebang, `.bats`, or the bash
  default). It's ignored for sh, dash, ksh and busybox, where the SC3xxx checks
  already cover portability.
- A bad `--bash-version` value exits with status 3. A bad rc value is an error under
  `--strict-rc` and ignored otherwise. A bad directive value gives SC1146.
- If teammates run the real ShellCheck on the same files, prefer the rc key. ShellCheck
  ignores an unknown rc key, but it reports the `# shellcheck bash-version=` directive
  as SC1107 (unknown directive) and exits 1.
- A version test in the script protects the code it guards, so the features it
  proves are not reported there. Inside `if ((BASH_VERSINFO[0] >= 4)); then ... fi`
  the target is 4.0, and `declare -A` is silent at `--bash-version=3.2`. Only the
  guard's region is affected, and only the features newer than the guard still warn.
  - Operands: `BASH_VERSINFO[0]`, `${BASH_VERSINFO[0]}`, `$BASH_VERSINFO`, a bare
    `BASH_VERSINFO` in `(( ))` and `[[ ]]`, `${BASH_VERSION%%.*}` and
    `${BASH_VERSION:0:1}` (one digit, so it is right up to bash 9). `BASH_VERSINFO[1]`
    is the minor version.
  - Comparisons with an integer literal, on either side: `>= > == < <=` in `(( ))`,
    `-ge -gt -eq -lt -le` and `==` in `[[ ]]`, and `-ge -gt -eq -lt -le` in `[ ]` and
    `test`. `[ BASH_VERSINFO -ge 4 ]` is not a guard, since bash reads the word
    `BASH_VERSINFO` there, not the version.
  - Combined major and minor: `(( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 3) ))`
    proves 4.3.
  - Regions: the body of `if G; then`, each `elif`, the `else` of a negated guard,
    the right side of `G && cmd` and of `NOT_G || cmd`, and every later statement
    in the same list after an early exit. The early exits are
    `if NOT_G; then ...; exit; fi` (or `return`), `NOT_G && { ...; exit; }` and
    `G || exit`. The exit must be the last command of its body. Guards nest, and a
    function defined inside a region is covered by it.
  - Not recognised, so still reported: a guard held in a variable
    (`v=${BASH_VERSINFO[0]}; ((v >= 4))`), `case` on `BASH_VERSION`, a non-literal
    bound, `!=`, an exit in a subshell or one followed by more statements in its
    body, and `>=` in `[[ ]]`, which bash rejects as a syntax error.
  - A guard only lowers what is reported. A 4.0 guard does not cover a 4.3 feature,
    which still warns with `(target is 3.2)`.
- Each finding is a warning, `declare -A requires bash 4.0 (target is 3.2).`, reported
  at every use. They're not optional checks, so `-o all` and the optional check list are unchanged.
  Each code has its own number so a project can turn off just the feature it polyfills,
  with `--exclude=SC9102` or `disable=SC9102`.

Not covered: behaviour changes that aren't syntax, such as the 3.2 quoted right-hand
side of `=~`, `**` without a visible `shopt -s globstar`, and the stepped brace
form `{1..9..2}`, whose release bash's NEWS doesn't state.

SC9100 to SC9199 are shelllint's own.

| Code | Needs bash | Feature |
| --- | --- | --- |
| SC9100 | 3.1 | `printf -v var` |
| SC9101 | 3.1 | `+=` append to a variable or array |
| SC9102 | 4.0 | `declare -A`, `local -A`, `typeset -A` (associative arrays) |
| SC9103 | 4.0 | `mapfile`, `readarray` |
| SC9104 | 4.0 | `\|&` (pipe stderr too) |
| SC9105 | 4.0 | `;&` and `;;&` case terminators |
| SC9106 | 4.0 | `coproc` |
| SC9107 | 4.0 | `${v,}` `${v,,}` `${v^}` `${v^^}` case modification |
| SC9108 | 4.0 | Zero-padded brace expansion, `{01..10}` |
| SC9109 | 4.0 | `&>>` |
| SC9110 | 4.0 | `$BASHPID` |
| SC9111 | 4.0 | `shopt -s globstar` |
| SC9112 | 4.0 | `read -t` with a fractional timeout |
| SC9113 | 4.1 | `{fd}>` fd allocation |
| SC9114 | 4.1 | `read -N` |
| SC9115 | 4.1 | `printf -v 'a[i]'` (an array element) |
| SC9116 | 4.2 | `declare -g` |
| SC9117 | 4.2 | Negative index or length in `${a[-1]}` and `${v:0:-1}` |
| SC9118 | 4.2 | `[[ -v var ]]`, `test -v var` |
| SC9119 | 4.2 | `shopt -s lastpipe` |
| SC9120 | 4.2 | `printf '%(fmt)T'` |
| SC9121 | 4.3 | Negative index assignment or unset, `a[-1]=x` |
| SC9122 | 4.3 | `declare -n`, `local -n` (namerefs) |
| SC9123 | 4.3 | `[[ -R var ]]` |
| SC9124 | 4.3 | `wait -n` |
| SC9125 | 4.4 | `${v@Q}` `@E` `@P` `@A` `@a` |
| SC9126 | 4.4 | `local -` |
| SC9127 | 4.4 | `mapfile -d` |
| SC9128 | 4.4 | `shopt -s inherit_errexit` |
| SC9129 | 5.0 | `$EPOCHSECONDS`, `$EPOCHREALTIME`, `$BASH_ARGV0` |
| SC9130 | 5.0 | `wait -f` |
| SC9131 | 5.0 | `shopt -s localvar_inherit`, `localvar_unset` |
| SC9132 | 5.1 | `$SRANDOM` |
| SC9133 | 5.1 | `wait -p` |
| SC9134 | 5.1 | `${v@U}` `@u` `@L` `@K` |
| SC9135 | 5.2 | `${v@k}` |
| SC9136 | 5.2 | `shopt -s patsub_replacement` |
| SC9137 | 5.3 | `${ cmd; }` and `${\| cmd; }` |

## zsh

ShellCheck rejects zsh scripts with SC1071. shelllint lints them. A file is zsh when
its shebang names zsh, when `--shell=zsh`, `shell=zsh` in `.shellcheckrc` or
`# shellcheck shell=zsh` says so, or when it has no shebang and is named `*.zsh`,
`*.zsh-theme`, `.zshrc`, `.zshenv`, `.zprofile`, `.zlogin` or `.zlogout`.

This is experimental. There is no upstream zsh mode to compare against, so the
results are checked against zsh 5.9 itself and against the 567 `.zsh` files of a
popular plugin framework, which all parse. A construct outside that corpus may still
get a parse error. Please report it.

**Syntax.** The parser accepts zsh forms such as `for f (a b) cmd`, short
`if`/`while`/`for` bodies, `repeat N`, `function a b { }`, anonymous functions
`() { }`, `${(flags)x}`, `${=x}`, `${~x}`, nested `${${x}#a}`, glob qualifiers like
`*(.N)`, the `;|` and `&|` case terminators, and zsh redirections (`>|`, `>!`).

**Options.** zsh doesn't split or glob an unquoted `$x` unless options say so.
shelllint follows `setopt`, `unsetopt`, `set -o`/`+o`, `emulate [-L] [-R] zsh|sh|ksh`
and `options[name]=on|off` along the code, with zsh's spelling rules (case and
underscores ignored, a `no` prefix negates). A function keeps its option changes local
only under `localoptions` or `emulate -L`. Each file starts from zsh defaults.
Options are tracked where code is written, not where it's called from.

**Checks.** No zsh-only checks are added. Existing checks fire when their warning is
true in zsh:

| Behaviour under zsh | Codes |
|---|---|
| Same as for bash | SC2046, SC2155, SC2207 (zsh wording), SC2006, SC2091, SC2164 and most others |
| Only when `shwordsplit` is on, or on `${=x}` | SC2086 (splitting), SC2206, SC2048, SC2068, SC2223 |
| Only when `globsubst` is on, or on `${~x}` | SC2086 (globbing), SC2053, SC2295, SC2254 |
| Only when `ksharrays` is on | SC2128, SC2178, SC1087 |
| Only when `bsdecho` is on | SC2028 |
| zsh-aware | SC2154 and SC2034 know zsh's special parameters and hook arrays. SC2016 accepts single-quoted prompts under `promptsubst`. SC2329, SC2119 and SC2120 count `add-zsh-hook`, `zle -N`, `compdef`, `*_functions+=` and `TRAP*` as uses. SC2004 knows `$+name`. SC2162 knows zsh `read` flags |
| Off, as they have no zsh-true form | SC2296, SC2298, SC2299, SC2300, SC2301, SC2082, SC2190, SC2051, SC2277, SC2270, SC2168, SC2180, SC2338, SC2076, SC2079 |

SC2086 still fires inside `[ ]` and `test` by default. zsh drops an empty unquoted
word there too, so `x=; [ $x = y ]` fails with "condition expected". `[[ ]]` is safe.

A sourced file is linted in the sourcing script's shell. Completion specs
(`_arguments`) aren't interpreted.

## Containers

An Alpine image with a shell is published alongside the
scratch image, which suits CI systems like GitLab that need a shell:

```yaml
image:
  name: ghcr.io/devanshchoubey/shelllint:alpine
  entrypoint: [""]
```
