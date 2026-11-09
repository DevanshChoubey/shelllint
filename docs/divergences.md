# Intentional divergences from the Haskell ShellCheck (CLI and runtime)

Most divergences are marked in the code with a `// DIVERGENCE:` comment
(`grep -rn 'DIVERGENCE:' crates/`). The ones below are about how the upstream
binary fails or depends on its environment. They are kept on purpose because the
Haskell behaviour is a crash or an accident of the runtime, and copying it
would make the port worse without making it more correct.

## Where the oracle crashes and we do not

- **Empty numeric argument** (`-e SC`, `-i SC`, `-W ''`, `-W SC`). Upstream's
  `parseNum` accepts the empty string (`all isDigit ""` is true) and then
  `read ""` throws, so it prints `shellcheck: Prelude.read: no parse` and exits
  1. We print `Invalid number: ` and exit 3.
- **Locale encoding.** GHC encodes stdout and stderr with the locale codec, so
  under `LANG=C`, or when a surrogate such as `\xed\xa0\x80` reaches `-f tty`
  or `-f diff`, upstream stops mid-line with
  `<stdout>: commitBuffer: invalid argument (cannot encode character ...)` and
  exits 2. We always write UTF-8, and show a lone surrogate as U+FFFD.
- **`--files-from` with invalid UTF-8.** Upstream reads the list lazily with the
  locale decoder and the exception escapes: no output,
  `shellcheck: l: hGetContents: invalid argument (cannot decode byte sequence
  starting from 255)`, exit 1. We decode the list leniently, report the
  undecodable name as missing (exit 2) and still check the other files.

## Behaviour that changes without a flag

These differ from ShellCheck even in a run that uses no shelllint option. The
parity corpus doesn't exercise them.

- **`nix-shell` shebangs.** `#!/usr/bin/env nix-shell` no longer gives SC1008.
  The shell comes from the `#! nix-shell -i <shell>` lines directly after the
  shebang, or bash if there is no `-i`. Non-shell interpreters keep SC1008.
- **Home expansion.** A leading `~`, `$HOME` or `${HOME}` is expanded in
  `source-path=`, `-P` and `# shellcheck source=`.
- **`RCDIR`.** A leading `RCDIR` component in `source-path=` in an rc file means
  the rc file's directory. A real directory named `RCDIR` needs `./RCDIR`.
- **New directive and rc keys** (`severity`, `external-vars`, `source-functions`,
  `disable-start`, `disable-end`, `disable-line`, `color`, `wiki-link-count`,
  `modern-dialects`).
  ShellCheck reports or ignores these as unknown. An invalid `severity=` in a
  directive gives SC1147, a code ShellCheck doesn't use.
- **Repeated `source`.** Mutually sourcing scripts are expanded at most 512
  times per checked script, then repeats are treated as empty with no
  diagnostic. This is what stops the runaway memory use. Normal scripts never
  reach the limit.
- **`--help` and the format list** also show shelllint's own options and formats.
  ShellCheck's rows and column layout are unchanged.
- **Abbreviated long options** resolve against ShellCheck's own options first, so
  `--ver` is still `--version`. Otherwise they can reach a shelllint option.

## Related notes

- **Lone surrogates in source text.** Haskell's `decodeString` turns
  `ED A0 80` into `chr 0xD800`; a Rust `char` cannot hold that, so it becomes
  U+FFFD. Positions and codes match, only message text differs.
- **`T_CoProc` as the last pipeline element.** Upstream's `checkCmd` has no
  fallback case and dies with `Non-exhaustive patterns`; we ignore it.
- **Out of memory.** GHC prints `<prog>: out of memory` and exits 251. The CLI
  binary installs a global allocator wrapper (`crates/cli/src/oom.rs`) that
  prints `shelllint: out of memory` and exits 251. Library users keep
  Rust's default abort behaviour.
- **File-open error text.** `show_io_error` reproduces GHC's `IOErrorType`
  wording from `errnoToIOError` together with the musl `strerror` text that the
  reference binary prints (e.g. `invalid argument (Symbolic link loop)`).
  Errnos GHC does not classify print `failed (<text>)`; for those the text comes
  from the host libc and may differ from upstream's.
- **Whitespace.** `--files-from` lines are trimmed with GHC's `isSpace`
  (`shelllint_core::unicode::is_space`): ASCII whitespace, U+00A0 and Unicode
  space separators, but not U+0085, U+2028 or U+2029.
